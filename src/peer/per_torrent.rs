use std::sync::atomic::AtomicU32;

use anyhow::bail;
use bytepack::base::ConstByteSize;
use bytepack::pack::BytePack;
use bytepack::unpack::ByteUnpack;
use peer_message::PeerMessage;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, Receiver, Sender};

use super::peer_net;
use super::*;
use crate::*;

// TODO: impl proper piece requester
pub async fn spin_piece_requester(
    torrent_file: Global<TorrentFile>,
    peer_list: Global<PeerList>,
    send_channels: Global<SendChannels>,
    // mut go_ahead_channel: Receiver<()>,
    // mut start_sweep_channel: Receiver<()>,
    on_fly_request_count: Arc<AtomicU32>,
) {
    println!("spin_piece_requester");

    let mut go_ahead_channel: Receiver<()> = unimpl_create();
    let mut start_sweep_channel: Receiver<()> = unimpl_create();

    // (availability, piece_index)
    let mut piece_indices: Vec<(u32, usize)> = Vec::new();

    loop {
        piece_indices.clear();
        let Some(()) = start_sweep_channel.recv().await else {
            return; // channel closed
        };
        println!("Sweep started for torrent requesting...");

        {
            let torrent_file_read = torrent_file.read().await;
            let pieces = torrent_file_read
                .bitfield
                .iter()
                .enumerate()
                .filter(|(_, b)| **b)
                .map(|(piece_index, _)| (0, piece_index));
            piece_indices.extend(pieces);
        };

        if piece_indices.is_empty() {
            return; // downloaded all
        }

        {
            let peer_list_read = peer_list.read().await;
            for (availability, piece_index) in &mut piece_indices {
                let piece_index = *piece_index;
                for peer in peer_list_read.peers.iter() {
                    let peer_read = peer.read().await;
                    *availability += *peer_read.bitfield.get(piece_index).unwrap() as u32;
                }
            }
        }
        piece_indices.sort_by_key(|(availability, _)| *availability);

        for (_, piece_index) in piece_indices.iter().cloned() {
            let peer = {
                let peer_list_read = peer_list.read().await;
                let mut found = false;
                let mut peer_index = 0;
                for (i, peer) in peer_list_read.peers.iter().enumerate() {
                    let peer_read = peer.read().await;
                    if *peer_read.bitfield.get(piece_index).unwrap() {
                        found = true;
                        peer_index = i;
                        break;
                    }
                }
                if !found {
                    continue;
                }
                peer_list_read.get_peer_by_index(peer_index)
            };

            let (peer_id, peer_index) = {
                let peer_read = peer.read().await;
                (peer_read.peer_id, peer_read.index)
            };

            let piece_chunks = {
                let torrent_file_read = torrent_file.read().await;
                torrent_file_read
                    .write_half
                    .get_piece_chunks(piece_index)
            };

            let send_channel = {
                let send_channels_read = send_channels.read().await;
                send_channels_read.get(&peer_index).unwrap().clone()
            };

            for chunk in piece_chunks {
                let request = peer_message::Request {
                    index: piece_index as u32,
                    begin: chunk.begin as u32,
                    length: chunk.length as u32,
                };
                let Ok(()) = send_channel
                    .send(SendPeerMessage {
                        peer_id,
                        message: PeerMessage::Request(request.clone()),
                    })
                    .await
                else {
                    return; // channel closed
                };
                {
                    let mut peer_write = peer.write().await;
                    peer_write.state.waiting_on = Some(request);
                };
                println!("Waiting for go ahead...");
                let Some(()) = go_ahead_channel.recv().await else {
                    return; // channel closed
                };
                println!("Go ahead received");
            }
        }
    }
}

pub async fn spin_handle_piece_upload(
    torrent_file: Global<TorrentFile>,
    peer_list: Global<PeerList>,
    send_channels: Global<SendChannels>,
    cancelled_requests: Global<CancelledRequests>,
    mut piece_request_queue: Receiver<PieceRequestItem>,
) {
    loop {
        let Some(piece_request) = piece_request_queue.recv().await else {
            return; // channel closed
        };

        let cancel_counterpart = piece_request.into_cancel();
        let is_cancelled = {
            let cancelled_requests_read = cancelled_requests.read().await;
            cancelled_requests_read.contains(&cancel_counterpart)
        };
        if is_cancelled {
            let mut cancelled_requests_write = cancelled_requests.write().await;
            cancelled_requests_write.remove(&cancel_counterpart);
            continue;
        }

        let peer_index = piece_request.peer_index;
        let request = piece_request.request;

        let Ok(chunk) = ({
            let mut torrent_file_write = torrent_file.write().await;
            torrent_file_write
                .read_half
                .read_piece_chunk(
                    request.index as usize,
                    request.begin as usize,
                    request.length as usize,
                )
                .await
        }) else {
            return;
        };

        let peer = {
            let peer_list_read = peer_list.read().await;
            peer_list_read.get_peer_by_index(peer_index)
        };

        let peer_id = {
            let peer_read = peer.read().await;
            peer_read.peer_id
        };

        let send_channel = {
            let send_channels_read = send_channels.read().await;
            send_channels_read.get(&peer_index).unwrap().clone()
        };

        let Ok(()) = send_channel
            .send(SendPeerMessage {
                peer_id,
                message: PeerMessage::Piece(peer_message::Piece {
                    index: request.index,
                    begin: request.begin,
                    chunk: bytepack::base::SplatDrain::Splat(chunk.into()),
                }),
            })
            .await
        else {
            return;
        };

        {
            let mut peer_write = peer.write().await;
            peer_write.stats.download += request.length as u64;
        };
    }
}

pub async fn initiate_peer_connection(
    self_peer_id: PeerId,
    this_torrent_info_hash: InfoHash,
    init_peer_id: PeerId,
    peer_addr: SocketAddr,
) -> anyhow::Result<(PeerId, TcpStream)> {
    let mut net_buffer = [0; Handshake::const_hold_size()];

    let mut stream = TcpStream::connect(peer_addr).await?;

    let handshake_request = Handshake::new(&this_torrent_info_hash.hash, &self_peer_id.id);
    handshake_request.pack(net_buffer.as_mut()).unwrap();
    stream
        .write_all(&net_buffer[..Handshake::const_byte_size()])
        .await?;

    stream
        .read_exact(&mut net_buffer[..Handshake::const_byte_size()])
        .await?;
    let Ok(handshake_response) = Handshake::unpack(net_buffer.as_ref()) else {
        bail!("Handshake could not be parsed.");
    };

    // println!("Client");
    // dbg!(&handshake_response);

    let peer_id = PeerId {
        id: handshake_request.peer_id,
    };

    if !init_peer_id.is_zeroed() && init_peer_id != peer_id {
        bail!(
            "Expected peer_id [{}] and received peer_id [{}] do not match.",
            init_peer_id.as_str(),
            peer_id.as_str()
        );
    }

    // TODO: send bitfield if applicable

    Ok((peer_id, stream))
}

pub async fn spin_peer_receiver(
    self_peer_id: PeerId,
    single_torrent: Arc<SingleTorrent>,
    peer_list: Global<PeerList>,
    send_channels: Global<SendChannels>,
    received_message_channel_sender: Sender<ReceivedPeerMessage>,
    mut peer_add_channel: Receiver<PeerAddEvent>,
) {
    loop {
        println!("waiting peer_addrs...");
        let Some(peer_add_event) = peer_add_channel.recv().await else {
            break; // channel closed
        };
        println!("adding peer_addrs...");

        let mut peers = Vec::new();

        match peer_add_event {
            PeerAddEvent::Init(peer_addrs) => {
                let mut connected_peers_joins = Vec::with_capacity(peer_addrs.len());

                for peer_addr in peer_addrs.iter().cloned() {
                    let single_torrent_clone = single_torrent.clone();

                    connected_peers_joins.push(tokio::spawn(async move {
                        let init_peer_id = PeerId::uninit();

                        let (peer_id, stream) = match initiate_peer_connection(
                            self_peer_id,
                            single_torrent_clone.info_hash,
                            init_peer_id,
                            peer_addr,
                        )
                        .await
                        {
                            Ok(ret) => ret,
                            Err(err) => {
                                dbg!(err);
                                return Err(());
                            }
                        };

                        Ok((
                            Peer::new_connected(peer_id, peer_addr, &single_torrent_clone),
                            stream,
                        ))
                    }));
                }

                for connected_peer_join in connected_peers_joins {
                    if let Ok(Ok(peer)) = connected_peer_join.await {
                        peers.push(peer);
                    }
                    // TODO: add retry for Err peers
                }
            }
            PeerAddEvent::Connected(peer_id, peer_addr, tcp_stream) => {
                let peer = Peer::new_connected(peer_id, peer_addr, &single_torrent);
                peers.push((peer, tcp_stream));
            }
        }

        let piece_length = single_torrent.piece_length;
        {
            let mut peer_list_write = peer_list.write().await;
            for (peer, stream) in peers {
                let (peer_index, peer) = peer_list_write.add_peer(peer);

                let peer_clone = peer.clone();
                let send_channels_clone = send_channels.clone();
                let received_message_channel_sender_clone = received_message_channel_sender.clone();

                tokio::spawn(async move {
                    let (send_message_channel_sender, send_message_channel_receiver) =
                        mpsc::channel::<SendPeerMessage>(10);

                    {
                        let mut send_channels_write = send_channels_clone.write().await;
                        send_channels_write.insert(peer_index, send_message_channel_sender);
                    }

                    let (read_stream, write_stream) = stream.into_split();

                    // TODO: error handle
                    let _ = tokio::join!(
                        peer_net::spin_receive_peer_message(
                            peer_clone.clone(),
                            received_message_channel_sender_clone,
                            read_stream,
                            create_peer_network_buffer(piece_length),
                        ),
                        peer_net::spin_send_peer_message(
                            peer_clone.clone(),
                            send_message_channel_receiver,
                            write_stream,
                            create_peer_network_buffer(piece_length),
                        )
                    );
                });
            }
        }
    }
}
