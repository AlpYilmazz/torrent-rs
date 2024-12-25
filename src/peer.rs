use core::str;
use std::{
    cell::RefCell,
    collections::HashMap,
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
    util::UnifyError,
    TorrentContext,
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

const BUFFER_SIZE: usize = 10000;
const MAX_UNCHOKED_COUNT: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId {
    pub id: [u8; 20],
}

impl PeerId {
    pub fn uninit() -> Self {
        Self { id: [0; 20] }
    }

    pub fn is_zeroed(&self) -> bool {
        self.id.iter().all(|e| *e == 0)
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.id) }
    }
}

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

pub struct PeerPieceContext {
    pub piece_count: u32,
    pub piece_hashes: Arc<[String]>,
    pub bitfield: Vec<bool>,
}

impl PeerPieceContext {
    pub fn init(piece_hashes: Arc<[String]>) -> Self {
        let piece_count = piece_hashes.len();
        Self {
            piece_count: piece_count as u32,
            piece_hashes,
            bitfield: vec![false; piece_count],
        }
    }
}

pub struct Peer {
    pub peer_id: PeerId,
    pub addr: SocketAddr,
    pub state: PeerState,
    pub piece_context: PeerPieceContext,
    pub terminate: bool,
}

impl Peer {
    pub fn new(addr: SocketAddr, piece_hashes: Arc<[String]>) -> Self {
        Self {
            peer_id: PeerId::uninit(),
            addr,
            state: PeerState::init(),
            piece_context: PeerPieceContext::init(piece_hashes),
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

    pub fn add_peers(
        &mut self,
        piece_hashes: Arc<[String]>,
        peers: impl IntoIterator<Item = SocketAddr>,
    ) {
        for addr in peers {
            if !self.addrs.contains(&addr) {
                self.peers
                    .push(Arc::new(RwLock::new(Peer::new(addr, piece_hashes.clone()))));
            }
        }
    }
}

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

pub struct PeerConnections {
    // pub connections: HashMap<PeerId, TcpStream>,
    pub read_streams: HashMap<PeerId, OwnedReadHalf>,
    pub write_streams: HashMap<PeerId, OwnedWriteHalf>,
}

impl PeerConnections {
    pub fn new() -> Self {
        Self {
            read_streams: HashMap::new(),
            write_streams: HashMap::new(),
        }
    }
}

pub async fn handle_peer_handshake(
    context: Arc<TorrentContext>,
    peer_addr: SocketAddr,
    mut stream: TcpStream,
) -> anyhow::Result<()> {
    let mut net_buffer = Box::new([0; 10000]);

    stream
        .read_exact(&mut net_buffer[..Handshake::const_byte_size()])
        .await?;
    let Ok(handshake_request) = Handshake::unpack(net_buffer.as_ref()) else {
        bail!("Handshake could not be parsed.");
    };

    println!("Server received from [peer_addr: {:?}]", peer_addr);
    dbg!(&handshake_request);

    let handshake_response = Handshake::new(&context.info_hash, &context.self_peer_id);
    handshake_response.pack(net_buffer.as_mut()).unwrap();
    stream.write_all(&net_buffer[..Handshake::const_byte_size()]).await?;

    Ok(())
}

pub async fn spin_peer_server(
    torrect_context: Arc<TorrentContext>,
    peer_addrs: Arc<std::sync::Mutex<Vec<SocketAddr>>>,
    listener: TcpListener,
) {
    loop {
        let (stream, peer_addr) = listener.accept().await.unwrap();
        tokio::spawn(handle_peer_handshake(
            torrect_context.clone(),
            peer_addr,
            stream,
        ));
    }
}

pub async fn initiate_peer_connection(
    context: Arc<TorrentContext>,
    net_buffer: &mut [u8],
    // peer: &mut Peer,
    init_peer_id: PeerId,
    peer_addr: SocketAddr,
) -> anyhow::Result<(PeerId, TcpStream)> {
    let mut stream = TcpStream::connect(peer_addr).await?;

    let handshake_request = Handshake::new(&context.info_hash, &context.self_peer_id);
    handshake_request.pack(net_buffer.as_mut()).unwrap();
    stream.write_all(&net_buffer[..Handshake::const_byte_size()]).await?;

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

pub struct ReceivedPeerMessage {
    pub peer_id: PeerId,
    pub message: PeerMessage,
}

pub async fn peer_handle_main(
    torrect_context: Arc<TorrentContext>,
    peer_addrs: Arc<std::sync::Mutex<Vec<SocketAddr>>>,
    peer_list: Arc<RwLock<PeerList>>,
    peer_connections: PeerConnections,
) {
    let (message_channel_sender, message_channel_receiver) =
        mpsc::channel::<ReceivedPeerMessage>(100);
    let mut peer_connections = Arc::new(RwLock::new(peer_connections));

    tokio::spawn(spin_handle_peer_messages(message_channel_receiver));

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(1 * 1000)).await;

        let mut peer_list_write = peer_list.write().await;
        {
            let mut peer_addrs = peer_addrs.lock().unwrap();
            peer_list_write.add_peers(torrect_context.piece_hashes.clone(), peer_addrs.drain(..));
        }

        for peer in &peer_list_write.peers {
            let (peer_connected, peer_id, peer_addr) = {
                let peer = peer.read().await;
                (peer.state.connected, peer.peer_id, peer.addr)
            };
            if peer_connected {
                continue;
            }

            let peer_clone = peer.clone();
            let torrect_context_clone = torrect_context.clone();
            // let peer_connections_clone = peer_connections.clone();
            let message_channel_sender_clone = message_channel_sender.clone();

            tokio::spawn(async move {
                let mut net_buffer = Box::new([0; 10000]);

                let (peer_id, stream) = match initiate_peer_connection(
                    torrect_context_clone,
                    net_buffer.as_mut_slice(),
                    peer_id,
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

                {
                    let mut peer_write = peer_clone.write().await;
                    peer_write.peer_id = peer_id;
                    peer_write.state.connected = true;
                }

                let (read_stream, write_stream) = stream.into_split();
                // {
                //     let mut peer_connections_write = peer_connections_clone.write().unwrap();
                //     peer_connections_write.read_streams.insert(peer_id, rd);
                //     peer_connections_write.write_streams.insert(peer_id, wt);
                // }

                // TODO: error handle
                let _ = tokio::join!(
                    spin_receive_from_peer(
                        peer_clone.clone(),
                        message_channel_sender_clone,
                        read_stream
                    ),
                    spin_peer_client(peer_clone.clone(), write_stream)
                );
            });
        }
    }
}

pub async fn spin_peer_client(peer: Arc<RwLock<Peer>>, mut write_stream: OwnedWriteHalf) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(2 * 1000)).await;
        println!(
            "Peer client [peer_id: {}]",
            peer.read().await.peer_id.as_str()
        );
    }
}

pub async fn spin_handle_peer_messages(
    mut message_channel: Receiver<ReceivedPeerMessage>,
) -> anyhow::Result<()> {
    loop {
        let Some(received_peer_message) = message_channel.recv().await else {
            break;
        };

        tokio::spawn(async move {
            println!(
                "Received from [peer_id: {}]: {:?}\n",
                received_peer_message.peer_id.as_str(),
                received_peer_message.message
            );
        });
    }

    Ok(())
}

pub async fn spin_receive_from_peer(
    peer: Arc<RwLock<Peer>>,
    message_channel: Sender<ReceivedPeerMessage>,
    mut read_stream: OwnedReadHalf,
) -> anyhow::Result<()> {
    let mut net_buffer = Box::new([0; 10000]);

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
                let bitfield_len = peer.piece_context.piece_count as usize;
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
                peer_id: peer.peer_id,
                message: peer_msg,
            })
            .await?;
    }

    Ok(())
}

// pub async fn spin_peer_server() -> std::io::Result<()> {
//     let listener = TcpListener::bind("0.0.0.0:8080").await?;
//     let (socket, _) = listener.accept().await?;
//     socket.into_split();

//     let local_addr = "0.0.0.0:8080".to_socket_addrs().unwrap().next().unwrap();

//     let sock = TcpSocket::new_v4()?;
//     sock.bind(local_addr)?;

//     Ok(())
// }

pub async fn test_peer_connection(
    peer: SocketAddr,
    context: Arc<TorrentContext>,
) -> anyhow::Result<()> {
    let local_addr = "0.0.0.0:8080".to_socket_addrs().unwrap().next().unwrap();

    let sock = TcpSocket::new_v4().unwrap();
    sock.bind(local_addr).unwrap();

    let mut buf = Box::new([0u8; 50000]);
    let mut stream = sock.connect(peer).await?;

    let handshake_request = Handshake::new(&context.info_hash, &context.self_peer_id);
    handshake_request.pack(buf.as_mut());
    stream.write_all(buf.as_ref()).await;

    let response_len = {
        let mut begin = 0;
        loop {
            begin = 0;
            loop {
                stream.readable().await?;
                match stream.try_read(&mut buf[begin..]) {
                    Ok(n) if n != 0 => {
                        begin += n;
                        continue;
                    }
                    Ok(_n) => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        Err(e)?;
                    }
                }
            }
            // if begin > 0 {
            //     break;
            // }
            tokio::time::sleep(std::time::Duration::from_millis(1 * 1000)).await;
        }
        begin
    };
    println!("response_len: {}", response_len);

    let handshake_response = Handshake::unpack(&buf[..response_len]).unwrap();

    dbg!(&handshake_response);
}
