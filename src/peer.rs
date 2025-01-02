use core::str;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
    rc::Rc,
    sync::Arc,
};

use anyhow::bail;
use bytepack::{
    base::{ByteSize, ConstByteSize, Throw},
    pack::BytePack,
    unpack::ByteUnpack,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpSocket, TcpStream,
    },
    sync::{
        mpsc::{self, Receiver, Sender},
        RwLock,
    },
};

use crate::{
    data::peer_message::{
        self, Handshake, PeerMessage, PEER_MESSAGE_BITFIELD, PEER_MESSAGE_CANCEL,
        PEER_MESSAGE_CHOKE, PEER_MESSAGE_HAVE, PEER_MESSAGE_INTERESTED, PEER_MESSAGE_NOTINTERESTED,
        PEER_MESSAGE_PIECE, PEER_MESSAGE_REQUEST, PEER_MESSAGE_UNCHOKE,
    },
    make_global,
    fileio::{PieceResult, TorrentFile},
    util::{self, UnifyError},
    Global, InfoHash, PeerId, SingleTorrent, TorrentCollection, TorrentContext, TorrentId,
};

// pub struct Peer {
//     pub addr: SocketAddr,
//     pub sock: TcpStream,
//     pub state: PeerState,
// }

// pub struct PeerClient {
//     peers: Vec<Peer>,
//     unchoked: Vec<usize>,
// }

// impl PeerClient {
//     pub async fn with_peers(peers_init: Vec<SocketAddr>) -> Result<Self, (usize, String)> {
//         let num_unchoked = usize::min(peers_init.len(), MAX_UNCHOKED_COUNT);
//         let mut peers = Vec::with_capacity(peers_init.len());

//         for addr in peers_init {
//             let mut sock = TcpStream::connect(addr.clone()).await.unify_error(1)?;
//             Self::handshake(&mut sock).await?;
//             peers.push(Peer {
//                 addr,
//                 sock,
//                 state: PeerState::init(),
//             });
//         }

//         Ok(Self {
//             peers,
//             unchoked: (0..num_unchoked).collect(),
//         })
//     }

//     async fn handshake(sock: &mut TcpStream) -> Result<(), (usize, String)> {
//         // sock.into_split()

//         Ok(())
//     }
// }

// pub struct _PeerClient {
//     ip: u32,
//     port: u16,
//     info_hash: Rc<[u8; 20]>,
//     peer_id: Rc<[u8; 20]>,
//     buffer: RefCell<[u8; BUFFER_SIZE]>,
// }

// impl _PeerClient {
//     pub fn new(ip: u32, port: u16, info_hash: Rc<[u8; 20]>, peer_id: Rc<[u8; 20]>) -> Self {
//         Self {
//             ip,
//             port,
//             info_hash,
//             peer_id,
//             buffer: RefCell::new([0; BUFFER_SIZE]),
//         }
//     }

//     pub async fn handshake(&self) -> Result<(), (usize, String)> {
//         let ip = "165.255.115.86".parse::<Ipv4Addr>().unwrap();
//         let port = 12242;
//         let mut sock =
//             TcpStream::connect(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(ip), port)))
//                 .await
//                 .unify_error(1)?;

//         // let handshake_request = Handshake {
//         //     protocol_len: 19,
//         //     protocol: Box::new(*b"BitTorrent protocol"),
//         //     reserved: Throw::new(),
//         //     info_hash: self.info_hash.clone(),
//         //     peer_id: self.peer_id.clone(),
//         // };
//         let handshake_request: Handshake = todo!();
//         let byte_size = handshake_request.byte_size();
//         let _res = handshake_request.pack(self.buffer.borrow_mut().as_mut_slice());

//         sock.write_all(&self.buffer.borrow().as_slice()[..byte_size])
//             .await
//             .unify_error(2)?;

//         let mut begin = 0;
//         loop {
//             sock.readable().await.unify_error(3)?;
//             match sock
//                 .read(&mut self.buffer.borrow_mut().as_mut_slice()[begin..])
//                 .await
//             {
//                 Ok(n) if n != 0 => {
//                     begin += n;
//                     continue;
//                 }
//                 Ok(_n) => break,
//                 Err(e) => return Err(e).unify_error(4),
//             }
//         }
//         let response_len = begin;
//         println!("response_len: {}", response_len);

//         let handshake_response =
//             Handshake::unpack(&self.buffer.borrow().as_slice()[..response_len]).unwrap();

//         dbg!(&handshake_response);

//         // let mut begin = 0;
//         // loop {
//         //     sock.writable().await.unify_error(2)?;
//         //     match sock.write(&self.buffer.borrow().as_slice()[begin..byte_size]).await {
//         //         Ok(n) if begin + n < byte_size => {
//         //             begin += n;
//         //             continue;
//         //         },
//         //         Ok(_n) => break,
//         //         Err(e) => return Err(e).unify_error(3),
//         //     }
//         // }

//         Ok(())
//     }
// }

// let br = BufReader::new(stream);
// br.read_exact(buf);

// let mut read_bytes = 0;
// loop {
//     stream.readable().await?;
//     match stream.try_read(&mut net_buffer[read_bytes..]) {
//         Ok(0) => break,
//         Ok(n) => {
//             read_bytes += n;
//             continue;
//         }
//         Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
//             continue;
//         }
//         Err(e) => {
//             Err(e)?;
//         }
//     }
// }
// let response_len = read_bytes;

// println!("response_len: {}", response_len);

// let Ok(handshake_response) = Handshake::unpack(&net_buffer[..response_len]) else {
//     bail!("Handshake could not be parsed.");
// };

const MAX_UNCHOKED_COUNT: usize = 4;

pub enum ChokeState {
    Choked,
    Unchoked,
}

pub enum InterestState {
    Interested,
    NotInterested,
}

pub struct PeerState {
    pub connected: bool,
    pub choke: ChokeState,
    pub interest: InterestState,
    pub waiting_on: Option<peer_message::Request>,
}

impl PeerState {
    pub fn init() -> Self {
        Self {
            connected: false,
            choke: ChokeState::Choked,
            interest: InterestState::NotInterested,
            waiting_on: None,
        }
    }
}

pub struct Peer {
    pub torrent_id: usize,
    pub index: usize,
    pub peer_id: PeerId,
    pub addr: SocketAddr,
    pub state: PeerState,
    pub piece_count: usize,
    pub bitfield: Vec<bool>,
    pub terminate: bool,
}

impl Peer {
    pub fn new_connected(peer_id: PeerId, addr: SocketAddr, st: &SingleTorrent) -> Self {
        let mut state = PeerState::init();
        state.connected = true;
        Self {
            torrent_id: st.id,
            index: 0,
            peer_id,
            addr,
            state,
            piece_count: st.piece_count,
            bitfield: vec![false; st.piece_count],
            terminate: false,
        }
    }

    pub fn new_uninit(addr: SocketAddr, st: &SingleTorrent) -> Self {
        Self {
            torrent_id: st.id,
            index: 0,
            peer_id: PeerId::uninit(),
            addr,
            state: PeerState::init(),
            piece_count: st.piece_count,
            bitfield: vec![false; st.piece_count],
            terminate: false,
        }
    }
}

pub struct PeerList {
    pub addrs: Vec<SocketAddr>,
    pub peers: Vec<Arc<RwLock<Peer>>>,
    pub unchoked: Vec<usize>,
}

impl PeerList {
    pub fn new() -> Self {
        Self {
            addrs: Vec::with_capacity(10),
            peers: Vec::with_capacity(10),
            unchoked: Vec::with_capacity(MAX_UNCHOKED_COUNT),
        }
    }

    pub fn add_peer(&mut self, mut peer: Peer) -> (usize, Arc<RwLock<Peer>>) {
        let peer_addr = peer.addr;

        let index = self.peers.len();
        peer.index = index;
        let peer = Arc::new(RwLock::new(peer));

        self.addrs.push(peer_addr);
        self.peers.push(peer);

        (index, self.peers[index].clone())
    }

    pub fn add_peers(&mut self, st: &SingleTorrent, peers: impl IntoIterator<Item = SocketAddr>) {
        for addr in peers {
            if !self.addrs.contains(&addr) {
                self.add_peer(Peer::new_uninit(addr, st));
            }
        }
    }

    pub fn get_peer_by_index(&self, index: usize) -> Arc<RwLock<Peer>> {
        self.peers[index].clone()
    }
}

pub struct PieceRequestItem {
    torrent_id: TorrentId,
    peer_index: usize,
    request: peer_message::Request,
    cancelled: bool,
}

// pub struct PieceRequestQueue {
//     queue: Vec<PieceRequestItem>,
// }

pub type PieceRequestQueue = VecDeque<PieceRequestItem>;

// pub struct Torrents {
//     pub peer_lists: HashMap<TorrentId, PeerList>,
// }

pub struct ReceivedPeerMessage {
    pub torrent_id: TorrentId,
    pub peer_index: usize,
    pub peer_id: PeerId,
    pub message: PeerMessage,
}

pub struct SendPeerMessage {
    pub peer_id: PeerId,
    pub message: PeerMessage,
}

pub type SendChannels = HashMap<usize, Sender<SendPeerMessage>>;

pub fn create_peer_network_buffer(piece_length: u32) -> Box<[u8]> {
    let len = 256 + piece_length as usize;
    util::create_buffer(len)
}

pub struct PeerConnectionInit {
    pub torrent_id: TorrentId,
    pub peer_id: PeerId,
}

pub async fn handle_peer_handshake(
    context: Global<TorrentContext>,
    peer_addr: SocketAddr,
    stream: &mut TcpStream,
) -> anyhow::Result<PeerConnectionInit> {
    let mut net_buffer = [0; Handshake::const_hold_size()];

    stream
        .read_exact(&mut net_buffer[..Handshake::const_byte_size()])
        .await?;
    let Ok(handshake_request) = Handshake::unpack(net_buffer.as_ref()) else {
        bail!("Handshake could not be parsed.");
    };

    println!("Server received from [peer_addr: {:?}]", peer_addr);
    dbg!(&handshake_request);

    let (torrent_id, self_peer_id) = {
        let context_read = context.read().await;
        let Some(torrent_id) = context_read
            .torrents
            .iter()
            .position(|st| st.info_hash.hash == handshake_request.info_hash)
        else {
            bail!("Requested Torrent doesnt exist on self peer");
        };
        (torrent_id, context_read.self_peer_id.clone())
    };

    let handshake_response = Handshake::new(&handshake_request.info_hash, &self_peer_id.id);
    handshake_response.pack(net_buffer.as_mut()).unwrap();
    stream
        .write_all(&net_buffer[..Handshake::const_byte_size()])
        .await?;

    Ok(PeerConnectionInit {
        torrent_id,
        peer_id: PeerId::from_raw(handshake_response.peer_id),
    })
}

pub async fn spin_peer_server(
    torrect_context: Global<TorrentContext>,
    peer_list: Global<PeerList>,
    send_channels: Global<SendChannels>,
    received_message_channel_sender: Sender<ReceivedPeerMessage>,
    listener: TcpListener,
) {
    loop {
        let (stream, peer_addr) = listener.accept().await.unwrap();

        let torrent_context_clone = torrect_context.clone();
        let peer_list_clone = peer_list.clone();
        let send_channels_clone = send_channels.clone();
        let message_channel_sender_clone = received_message_channel_sender.clone();

        tokio::spawn(async move {
            let mut stream = stream;

            let handshake_result =
                handle_peer_handshake(torrent_context_clone.clone(), peer_addr, &mut stream).await;

            let PeerConnectionInit {
                torrent_id,
                peer_id,
            } = match handshake_result {
                Ok(peer_connection_init) => peer_connection_init,
                Err(err) => {
                    dbg!(err);
                    return;
                }
            };

            let (send_message_channel_sender, send_message_channel_receiver) =
                mpsc::channel::<SendPeerMessage>(10);

            let (peer, torrent_piece_length) = {
                let torrent_context_read = torrent_context_clone.read().await;
                let st = torrent_context_read.torrents.get(torrent_id).unwrap();

                let mut peer_list_write = peer_list_clone.write().await;
                let (peer_index, peer) =
                    peer_list_write.add_peer(Peer::new_connected(peer_id, peer_addr, st));

                let mut send_channels_write = send_channels_clone.write().await;
                send_channels_write.insert(peer_index, send_message_channel_sender);

                (peer, st.piece_length)
            };

            let (read_stream, write_stream) = stream.into_split();

            // TODO: error handle
            let _ = tokio::join!(
                spin_receive_peer_message(
                    peer.clone(),
                    message_channel_sender_clone,
                    read_stream,
                    create_peer_network_buffer(torrent_piece_length)
                ),
                spin_send_peer_message(
                    peer.clone(),
                    send_message_channel_receiver,
                    write_stream,
                    create_peer_network_buffer(torrent_piece_length)
                )
            );
        });
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

    println!("Client");
    dbg!(&handshake_response);

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

    Ok((peer_id, stream))
}

pub async fn peer_handle_main(
    torrent_context: Global<TorrentContext>,
    torrent_collection: Global<TorrentCollection>,
    // peer_addrs: Arc<std::sync::Mutex<(TorrentId, Vec<SocketAddr>)>>,
    mut peer_addrs_channel: Receiver<(TorrentId, Arc<[SocketAddr]>)>,
) {
    let peer_list = make_global!(PeerList::new());
    let send_channels = make_global!(HashMap::<usize, Sender<SendPeerMessage>>::new());
    let piece_request_queue = make_global!(VecDeque::with_capacity(100));
    let server_listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    let (received_message_channel_sender, received_message_channel_receiver) =
        mpsc::channel::<ReceivedPeerMessage>(100);

    let message_handler_join = tokio::spawn(spin_handle_peer_messages(
        torrent_collection.clone(),
        peer_list.clone(),
        piece_request_queue.clone(),
        received_message_channel_receiver,
    ));

    let server_join = tokio::spawn(spin_peer_server(
        torrent_context.clone(),
        peer_list.clone(),
        send_channels.clone(),
        received_message_channel_sender.clone(),
        server_listener,
    ));

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(1 * 1000)).await;

        let Some((torrent_id, peer_addrs)) = peer_addrs_channel.recv().await else {
            break; // channel closed
        };

        {
            let mut peer_list_write = peer_list.write().await;
            let torrent_context_read = torrent_context.read().await;
            let st = torrent_context_read.torrents.get(torrent_id).unwrap();
            peer_list_write.add_peers(st, peer_addrs.iter().cloned());
        }

        let peer_list_read = peer_list.read().await;
        for peer in &peer_list_read.peers {
            let (peer_connected, torrent_id, init_peer_id, peer_addr) = {
                let peer = peer.read().await;
                (
                    peer.state.connected,
                    peer.torrent_id,
                    peer.peer_id,
                    peer.addr,
                )
            };
            if peer_connected {
                continue;
            }

            let peer_clone = peer.clone();
            let torrent_context_clone = torrent_context.clone();
            let send_channels_clone = send_channels.clone();
            let received_message_channel_sender_clone = received_message_channel_sender.clone();

            tokio::spawn(async move {
                let (self_peer_id, this_torrent_info_hash, piece_length) = {
                    let torrent_context_read = torrent_context_clone.read().await;
                    let st = torrent_context_read.torrents.get(torrent_id).unwrap();
                    (
                        torrent_context_read.self_peer_id,
                        st.info_hash,
                        st.piece_length,
                    )
                };

                let (peer_id, stream) = match initiate_peer_connection(
                    self_peer_id,
                    this_torrent_info_hash,
                    init_peer_id,
                    peer_addr,
                )
                .await
                {
                    Ok(ret) => ret,
                    Err(err) => {
                        dbg!(err);
                        return;
                    }
                };

                let (send_message_channel_sender, send_message_channel_receiver) =
                    mpsc::channel::<SendPeerMessage>(10);

                let peer_index = {
                    let mut peer_write = peer_clone.write().await;
                    peer_write.peer_id = peer_id;
                    peer_write.state.connected = true;
                    peer_write.index
                };
                {
                    let mut send_channels_write = send_channels_clone.write().await;
                    send_channels_write.insert(peer_index, send_message_channel_sender);
                }

                let (read_stream, write_stream) = stream.into_split();

                // TODO: error handle
                let _ = tokio::join!(
                    spin_receive_peer_message(
                        peer_clone.clone(),
                        received_message_channel_sender_clone,
                        read_stream,
                        create_peer_network_buffer(piece_length),
                    ),
                    spin_send_peer_message(
                        peer_clone.clone(),
                        send_message_channel_receiver,
                        write_stream,
                        create_peer_network_buffer(piece_length),
                    )
                );
            });
        }
    }
    // Peer addr channel has been closed
    // No new peers will arrive
    // But can still work with existing peers

    // TODO: what to do

    message_handler_join.await;
    server_join.await;
}

pub async fn spin_piece_requester(
    torrect_context: Global<TorrentContext>,
    peer_list: Global<PeerList>,
    send_channels: Global<SendChannels>,
) {
    // TODO
}

pub async fn spin_send_peer_message(
    peer: Global<Peer>,
    mut message_channel: Receiver<SendPeerMessage>,
    mut write_stream: OwnedWriteHalf,
    mut net_buffer: Box<[u8]>,
) {
    loop {
        let Some(send_peer_message) = message_channel.recv().await else {
            break; // channel closed
        };

        println!(
            "Sending to [peer_id: {}]: {:?}\n",
            send_peer_message.peer_id.as_str(),
            send_peer_message.message.message_type()
        );

        let message = send_peer_message.message;
        let message_size = message.byte_size();
        message.pack(&mut net_buffer).unwrap();

        write_stream
            .write_all(&net_buffer[..message_size])
            .await
            .unwrap();
    }
}

pub async fn handle_piece_request(piece_request_queue: Global<PieceRequestQueue>, peer: Global<Peer>, request: peer_message::Request) {
    let peer_choked = false; // TODO
    let peer_interested = true; // TODO
    if peer_choked || !peer_interested {
        return;
    }

    let (torrent_id, peer_index) = {
        let peer_read = peer.read().await;
        (peer_read.torrent_id, peer_read.index)
    };

    let mut piece_request_queue_write = piece_request_queue.write().await;

    let request_exists = piece_request_queue_write
        .iter()
        .any(|rq| {
            rq.peer_index == peer_index
                && rq.request.index == request.index
                && rq.request.begin == request.begin
        });

    if request_exists {
        // TODO: limit per peer requests
        piece_request_queue_write.push_back(PieceRequestItem {
            torrent_id,
            peer_index,
            request,
            cancelled: false,
        });
    }
}

fn unimpl_get_piece_hash() -> &'static str {
    todo!()
}
pub async fn handle_piece_download(
    torrent_file: Global<TorrentFile>,
    peer: Global<Peer>,
    piece: peer_message::Piece,
) {
    let piece_hash = unimpl_get_piece_hash();

    let mut torrent_file_write = torrent_file.write().await;
    torrent_file_write.write_piece_chunk(piece.index as usize, piece.begin as usize, &piece.piece);

    match torrent_file_write.check_piece(piece.index as usize, piece_hash) {
        PieceResult::Success => {
            torrent_file_write
                .flush_piece(piece.index as usize)
                .await
                .unwrap(); // TODO retry if fails
        }
        PieceResult::NotComplete => {}
        PieceResult::IntegrityFault => {
            torrent_file_write.reset_piece(piece.index as usize);
        }
    }
}

pub async fn handle_cancel_request(piece_request_queue: Global<PieceRequestQueue>, peer: Global<Peer>, cancel: peer_message::Cancel) {
    let peer_index = {
        let peer_read = peer.read().await;
        peer_read.index
    };

    let mut piece_request_queue_write = piece_request_queue.write().await;

    let Some(rq) =
        piece_request_queue_write
            .iter_mut()
            .find(|rq| {
                rq.peer_index == peer_index && rq.request.index == cancel.index && rq.request.begin == cancel.begin
            })
    else {
        return;
    };

    rq.cancelled = true;
}

pub async fn spin_handle_peer_messages(
    torrent_collection: Global<TorrentCollection>,
    peer_list: Global<PeerList>,
    piece_request_queue: Global<PieceRequestQueue>,
    mut message_channel: Receiver<ReceivedPeerMessage>,
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

        let peer = {
            let peer_list_read = peer_list.read().await;
            peer_list_read.get_peer_by_index(peer_index)
        };
        let torrent_file = {
            let torrent_collection_read = torrent_collection.read().await;
            torrent_collection_read.get(&torrent_id).unwrap().clone()
        };
        let piece_request_queue_clone = piece_request_queue.clone();

        tokio::spawn(async move {
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
                    peer_write.bitfield[have.index as usize] = true;
                }
                PeerMessage::Bitfield(bitfield) => {
                    let mut peer_write = peer.write().await;
                    peer_write.bitfield = bitfield
                        .bitfield
                        .into_vec()
                        .into_iter()
                        .map(|b| b == 1)
                        .collect();
                }
                PeerMessage::Request(request) => {
                    handle_piece_request(piece_request_queue_clone, peer, request).await;
                }
                PeerMessage::Piece(piece) => {
                    handle_piece_download(torrent_file, peer, piece).await;
                }
                PeerMessage::Cancel(cancel) => {
                    handle_cancel_request(piece_request_queue_clone, peer, cancel).await;
                }
            }
        });
    }

    Ok(())
}

pub async fn spin_receive_peer_message(
    peer: Global<Peer>,
    message_channel: Sender<ReceivedPeerMessage>,
    mut read_stream: OwnedReadHalf,
    mut net_buffer: Box<[u8]>,
) -> anyhow::Result<()> {
    loop {
        read_stream.read_exact(&mut net_buffer[..1]).await?;
        let msg_type = net_buffer[0];

        let net_buffer = &mut net_buffer[1..];

        let peer = peer.read().await;
        if peer.terminate {
            break;
        }

        let peer_msg = match msg_type {
            PEER_MESSAGE_CHOKE => PeerMessage::Choke,
            PEER_MESSAGE_UNCHOKE => PeerMessage::Unchoke,
            PEER_MESSAGE_INTERESTED => PeerMessage::Interested,
            PEER_MESSAGE_NOTINTERESTED => PeerMessage::NotInterested,
            PEER_MESSAGE_HAVE => {
                let have_len = peer_message::Have::const_byte_size();
                read_stream.read_exact(&mut net_buffer[..have_len]).await?;

                let Ok(pm_have) = peer_message::Have::unpack(net_buffer) else {
                    bail!("Faulty peer message [Have]");
                };
                PeerMessage::Have(pm_have)
            }
            PEER_MESSAGE_BITFIELD => {
                let bitfield_len = peer.piece_count as usize;
                read_stream
                    .read_exact(&mut net_buffer[..bitfield_len])
                    .await?;

                let Ok(pm_bitfield) = peer_message::Bitfield::unpack(net_buffer) else {
                    bail!("Faulty peer message [Bitfield]");
                };
                PeerMessage::Bitfield(pm_bitfield)
            }
            PEER_MESSAGE_REQUEST => {
                let request_len = peer_message::Request::const_byte_size();
                read_stream
                    .read_exact(&mut net_buffer[..request_len])
                    .await?;

                let Ok(pm_request) = peer_message::Request::unpack(net_buffer) else {
                    bail!("Faulty peer message [Request]");
                };
                PeerMessage::Request(pm_request)
            }
            PEER_MESSAGE_PIECE => {
                // TODO: can wait for multiple pieces, piece requests could be buffered
                let Some(piece_len) = peer.state.waiting_on.as_ref().map(|w| w.length) else {
                    bail!("Not expecting a piece to arrive");
                };
                let piece_len = u32::const_byte_size()      // piece_message.index
                    + u32::const_byte_size()    // piece_message.begin
                    + piece_len as usize; // piece_message.piece

                read_stream.read_exact(&mut net_buffer[..piece_len]).await?;

                let Ok(pm_piece) = peer_message::Piece::unpack(net_buffer) else {
                    bail!("Faulty peer message [Piece]");
                };
                PeerMessage::Piece(pm_piece)
            }
            PEER_MESSAGE_CANCEL => {
                let cancel_len = peer_message::Cancel::const_byte_size();
                read_stream
                    .read_exact(&mut net_buffer[..cancel_len])
                    .await?;

                let Ok(pm_cancel) = peer_message::Cancel::unpack(net_buffer) else {
                    bail!("Faulty peer message [Cancel]");
                };
                PeerMessage::Cancel(pm_cancel)
            }
            _ => bail!("Faulty peer message [msg_type]"),
        };

        message_channel
            .send(ReceivedPeerMessage {
                torrent_id: peer.torrent_id,
                peer_index: peer.index,
                peer_id: peer.peer_id,
                message: peer_msg,
            })
            .await?;
    }

    Ok(())
}
