
use bytepack::{base::ByteSize, pack::BytePack};
use fileio::PieceResult;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::*;
use super::*;

pub async fn handle_piece_request(
    peer_list: Global<PeerList>,
    peer: Global<Peer>,
    request: peer_message::Request,
    piece_request_queue: Sender<PieceRequestItem>,
) {
    let (peer_index, peer_interest) = {
        let peer_read = peer.read().await;
        (peer_read.index, peer_read.state.interest)
    };

    let peer_is_unchoked = {
        let peer_list_read = peer_list.read().await;
        peer_list_read.unchoked.contains(&peer_index)
    };
    let peer_is_interested = peer_interest == InterestState::Interested;

    if !peer_is_unchoked || !peer_is_interested {
        return;
    }

    piece_request_queue.send(PieceRequestItem {
        peer_index,
        request,
    }).await;
}

pub async fn handle_piece_download(
    torrent_file: Global<TorrentFile>,
    piece_hashes: Arc<[String]>,
    peer: Global<Peer>,
    piece: peer_message::Piece,
) {
    {
        let mut peer_write = peer.write().await;
        peer_write.state.waiting_on = None;
        peer_write.stats.upload += piece.chunk.len() as u64;
    };

    let Some(piece_hash) = piece_hashes.get(piece.index as usize) else {
        return;
    };

    let mut torrent_file_write = torrent_file.write().await;
    torrent_file_write.write_half.write_piece_chunk(
        piece.index as usize,
        piece.begin as usize,
        &piece.chunk,
    );

    match torrent_file_write
        .write_half
        .check_piece(piece.index as usize, piece_hash)
    {
        PieceResult::Success => {
            torrent_file_write
                .write_half
                .flush_piece(piece.index as usize)
                .await
                .unwrap(); // TODO retry if fails
            torrent_file_write.bitfield.set(piece.index as usize, true);
        }
        PieceResult::NotComplete => {}
        PieceResult::IntegrityFault => {
            torrent_file_write
                .write_half
                .reset_piece(piece.index as usize);
        }
    }
}

pub async fn handle_cancel_request(
    cancelled_requests: Global<CancelledRequests>,
    peer: Global<Peer>,
    cancel: peer_message::Cancel,
) {
    let peer_index = {
        let peer_read = peer.read().await;
        peer_read.index
    };

    let rc = RequestCancel { peer_index, cancel };

    let mut cancelled_requests_write = cancelled_requests.write().await;
    cancelled_requests_write.insert(rc);
}

pub async fn spin_handle_peer_messages(
    torrent_context: Global<TorrentContext>,
    torrent_collection: Global<TorrentCollection>,
    peer_list_collection: Global<PeerListCollection>,
    cancelled_requests: Global<CancelledRequests>,
    piece_request_queue: Sender<PieceRequestItem>,
    mut message_channel: Receiver<ReceivedPeerMessage>,
    // go_ahead_channel: Sender<()>,
    // start_sweep_channel: Sender<()>,
) -> anyhow::Result<()> {
    loop {
        let Some(received_peer_message) = message_channel.recv().await else {
            break; // channel closed
        };

        println!(
            "Received from [peer_id: {}]: {:?}\n",
            received_peer_message.peer_id.as_str(),
            received_peer_message.message.message_type()
        );

        let torrent_id = received_peer_message.torrent_id;
        let peer_index = received_peer_message.peer_index;
        let _peer_id = received_peer_message.peer_id;
        let message = received_peer_message.message;

        let piece_hashes = {
            let torrent_context_read = torrent_context.read().await;
            torrent_context_read.torrents[torrent_id]
                .piece_hashes
                .clone()
        };
        let torrent_file = {
            let torrent_collection_read = torrent_collection.read().await;
            torrent_collection_read.get(&torrent_id).unwrap().clone()
        };
        let peer_list = {
            let peer_list_collection = peer_list_collection.read().await;
            peer_list_collection.get(&torrent_id).unwrap().clone()
        };
        let peer = {
            let peer_list_read = peer_list.read().await;
            peer_list_read.get_peer_by_index(peer_index)
        };

        let cancelled_requests_clone = cancelled_requests.clone();
        let piece_request_queue_clone = piece_request_queue.clone();
        // let go_ahead_channel_clone = go_ahead_channel.clone();
        // let start_sweep_channel_clone = start_sweep_channel.clone();

        tokio::spawn(async move {
            println!("spawned message handler");
            let peer = peer;
            let torrent_file = torrent_file;
            match message {
                PeerMessage::Choke => {
                    let mut peer_write = peer.write().await;
                    peer_write.state.choke = ChokeState::Choked;
                }
                PeerMessage::Unchoke => {
                    let mut peer_write = peer.write().await;
                    peer_write.state.choke = ChokeState::Unchoked;
                }
                PeerMessage::Interested => {
                    let mut peer_write = peer.write().await;
                    peer_write.state.interest = InterestState::Interested;
                }
                PeerMessage::NotInterested => {
                    let mut peer_write = peer.write().await;
                    peer_write.state.interest = InterestState::NotInterested;
                }
                PeerMessage::Have(have) => {
                    let mut peer_write = peer.write().await;
                    peer_write.bitfield.set(have.index as usize, true);
                    // start_sweep_channel_clone.send(()).await.unwrap(); // TODO
                }
                PeerMessage::Bitfield(bitfield) => {
                    println!("bitfield message");
                    let mut peer_write = peer.write().await;
                    peer_write.bitfield.clear();
                    peer_write
                        .bitfield
                        .extend_from_raw_slice(&bitfield.bitfield);
                    // start_sweep_channel_clone.send(()).await.unwrap(); // TODO
                }
                PeerMessage::Request(request) => {
                    println!("request message");
                    handle_piece_request(peer_list, peer, request, piece_request_queue_clone).await;
                }
                PeerMessage::Piece(piece) => {
                    println!("piece message");
                    handle_piece_download(torrent_file, piece_hashes, peer, piece).await;
                    // go_ahead_channel_clone.send(()).await.unwrap(); // TODO
                }
                PeerMessage::Cancel(cancel) => {
                    handle_cancel_request(cancelled_requests_clone, peer, cancel).await;
                }
            }
        });
    }

    Ok(())
}