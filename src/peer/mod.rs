use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU32;

use bitvec::vec::BitVec;
use data::peer_message::PeerMessage;
use sha1::{Digest, Sha1};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;

use crate::{
    data::{
        metainfo::*,
        peer_message::{self, *},
    },
    util::*,
    *,
};

pub mod message_handle;
pub mod peer_net;
pub mod per_torrent;
pub mod server;
pub mod tracker;
pub mod unchoke;

const MAX_UNCHOKED_COUNT: usize = 4;

pub struct PeerStats {
    pub download: u64,
    pub upload: u64,
}

impl PeerStats {
    pub fn init() -> Self {
        Self {
            download: 0,
            upload: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChokeState {
    Choked,
    Unchoked,
}

#[derive(Clone, Copy, PartialEq, Eq)]
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
    pub bitfield: BitVec<u8>,
    pub stats: PeerStats,
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
            bitfield: BitVec::repeat(false, st.piece_count),
            stats: PeerStats::init(),
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
            bitfield: BitVec::repeat(false, st.piece_count),
            stats: PeerStats::init(),
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

pub type PeerListCollection = HashMap<TorrentId, Global<PeerList>>;

pub enum PeerAddEvent {
    Init(Arc<[SocketAddr]>),
    Connected(PeerId, SocketAddr, TcpStream),
}

pub type PeerAddEventChannels = HashMap<TorrentId, Sender<PeerAddEvent>>;

#[derive(Hash, Clone, PartialEq, Eq)]
pub struct PieceRequestItem {
    peer_index: usize,
    request: peer_message::Request,
}

impl PieceRequestItem {
    pub fn into_cancel(&self) -> RequestCancel {
        RequestCancel {
            peer_index: self.peer_index,
            cancel: peer_message::Cancel {
                index: self.request.index,
                begin: self.request.begin,
                length: self.request.length,
            },
        }
    }
}

#[derive(Hash, Clone, PartialEq, Eq)]
pub struct RequestCancel {
    peer_index: usize,
    cancel: peer_message::Cancel,
}
pub type CancelledRequests = HashSet<RequestCancel>;

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
pub type GoAheadChannels = HashMap<usize, Receiver<()>>;
pub type StartSweepChannels = HashMap<usize, Receiver<()>>;

pub fn create_peer_network_buffer(piece_length: u32) -> Box<[u8]> {
    let len = 256 + piece_length as usize;
    util::create_buffer(len)
}

pub async fn spin_torrent_add_handler(
    torrent_context: Global<TorrentContext>,
    torrent_collection: Global<TorrentCollection>,
    peer_list_collection: Global<PeerListCollection>,
    send_channels: Global<SendChannels>,
    peer_add_channels: Global<PeerAddEventChannels>,
    // --
    // go_ahead_channel_receiver: Receiver<()>,
    // start_sweep_channel_receiver: Receiver<()>,
    // --
    mut add_torrent_channel: Receiver<Metainfo>,
) {
    loop {
        let Some(metainfo) = add_torrent_channel.recv().await else {
            return; // channel closed
        };

        let info_bencoded = serde_bencode::to_bytes(&metainfo.info).unwrap();

        let mut sha1_hasher = Sha1::new();
        sha1_hasher.update(&info_bencoded);
        let info_hash: [u8; 20] = sha1_hasher.finalize().into();
        let info_hash = InfoHash::from_raw(info_hash);

        let piece_hashes = metainfo.info.get_pieces_as_sha1_hex();
        let piece_hashes_count = piece_hashes.len();

        let FileMode::SingleFile(single_file) = metainfo.info.mode else {
            println!("MultipleFiles torrents are not yet supported.");
            continue;
        };
        let file_length = single_file.length;

        let path = metainfo.info.name;
        let piece_count = piece_hashes_count;
        let piece_length = metainfo.info.piece_length;

        let torrent_exists = {
            let torrent_context_read = torrent_context.read().await;
            torrent_context_read
                .torrents
                .iter()
                .any(|t| t.info_hash == info_hash)
        };
        if torrent_exists {
            println!("Torrent described in metainfo already exists.");
            continue;
        }

        let Ok(torrent_file) =
            TorrentFile::new(&path, file_length, piece_count, piece_length as usize).await
        else {
            println!("TorrentFile from metainfo could not be created.");
            continue;
        };

        let torrent_file = make_global!(torrent_file);
        let peer_list = make_global!(PeerList::new());

        // TODO: should I global lock to add everything atomically
        let torrent_id = {
            let mut torrent_context_write = torrent_context.write().await;
            let torrent_id = torrent_context_write.torrents.len();
            let single_torrent = Arc::new(SingleTorrent {
                id: torrent_id,
                info_hash,
                piece_length,
                piece_count,
                piece_hashes: piece_hashes.into(),
            });
            torrent_context_write.torrents.push(single_torrent);
            torrent_id
        };

        {
            let mut torrent_collection_write = torrent_collection.write().await;
            torrent_collection_write.insert(torrent_id, torrent_file);
        }

        {
            let mut peer_list_collection_write = peer_list_collection.write().await;
            peer_list_collection_write.insert(torrent_id, peer_list);
        }

        spawn_torrent_handlers(
            torrent_id,
            torrent_context.clone(),
            torrent_collection.clone(),
            peer_list_collection.clone(),
            send_channels.clone(),
            peer_add_channels.clone(),
            // received_message_channel_sender.clone(),
            // cancelled_requests.clone(),
            // piece_request_queue.clone(),
            // go_ahead_channel_receiver,
            // start_sweep_channel_receiver,
        )
        .await;
    }
}

pub async fn spawn_torrent_handlers(
    torrent_id: TorrentId,
    torrent_context: Global<TorrentContext>,
    torrent_collection: Global<TorrentCollection>,
    peer_list_collection: Global<PeerListCollection>,
    send_channels: Global<SendChannels>,
    mut peer_add_channels: Global<PeerAddEventChannels>,
    // --
    // go_ahead_channel_receiver: Receiver<()>,
    // start_sweep_channel_receiver: Receiver<()>,
) {
    let (self_peer_id, single_torrent) = {
        let torrent_context_read = torrent_context.read().await;
        (
            torrent_context_read.self_peer_id,
            torrent_context_read
                .torrents
                .get(torrent_id)
                .unwrap()
                .clone(),
        )
    };
    let torrent_file = {
        let torrent_collection_read = torrent_collection.read().await;
        torrent_collection_read.get(&torrent_id).unwrap().clone()
    };
    let peer_list = {
        let peer_list_collection_read = peer_list_collection.read().await;
        peer_list_collection_read.get(&torrent_id).unwrap().clone()
    };

    let cancelled_requests = make_global!(HashSet::new());

    let (received_message_channel_sender, received_message_channel_receiver) =
        mpsc::channel::<ReceivedPeerMessage>(100);
    let (peer_add_channel_sender, peer_add_channel_receiver) = mpsc::channel::<PeerAddEvent>(100);
    let (piece_request_queue_sender, piece_request_queue_receiver) =
        mpsc::channel::<PieceRequestItem>(100);

    {
        let mut peer_add_channels_write = peer_add_channels.write().await;
        peer_add_channels_write.insert(torrent_id, peer_add_channel_sender.clone());
    }

    tokio::spawn(unchoke::spin_unchoke_strategy(
        torrent_id,
        peer_list.clone(),
    ));

    tokio::spawn(tracker::spin_tracker_client(peer_add_channel_sender));

    tokio::spawn(per_torrent::spin_peer_receiver(
        self_peer_id,
        single_torrent,
        peer_list.clone(),
        send_channels.clone(),
        received_message_channel_sender.clone(),
        peer_add_channel_receiver,
    ));

    tokio::spawn(message_handle::spin_handle_peer_messages(
        torrent_context.clone(),
        torrent_collection.clone(),
        peer_list_collection.clone(),
        cancelled_requests.clone(),
        piece_request_queue_sender,
        received_message_channel_receiver,
        // go_ahead_channel_sender,
        // start_sweep_channel_sender,
    ));

    tokio::spawn(per_torrent::spin_piece_requester(
        torrent_file.clone(),
        peer_list.clone(),
        send_channels.clone(),
        // go_ahead_channel_receiver,
        // start_sweep_channel_receiver,
        Arc::new(AtomicU32::new(0)), // TODO: on_fly_request_count
    ));

    tokio::spawn(per_torrent::spin_handle_piece_upload(
        torrent_file.clone(),
        peer_list.clone(),
        send_channels.clone(),
        cancelled_requests,
        piece_request_queue_receiver,
    ));
}

pub async fn peer_handle_main(add_torrent_channel: Receiver<Metainfo>) {
    const SELF_PEER_ID: &'static str = "123456789--qweasdzxc";

    let torrent_context = make_global!(TorrentContext::new(SELF_PEER_ID));
    let torrent_collection = make_global!(HashMap::new());
    let peer_list_collection = make_global!(HashMap::new());
    let send_channels = make_global!(HashMap::new());
    let peer_add_channels = make_global!(HashMap::new());

    let server_listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let torrent_add_handler_join = tokio::spawn(spin_torrent_add_handler(
        torrent_context.clone(),
        torrent_collection.clone(),
        peer_list_collection.clone(),
        send_channels,
        peer_add_channels.clone(),
        add_torrent_channel,
    ));

    let server_join = tokio::spawn(server::spin_peer_server(
        torrent_context.clone(),
        torrent_collection.clone(),
        peer_add_channels,
        server_listener,
    ));

    let _ = torrent_add_handler_join.await;
    let _ = server_join.await;
}
