use std::{array, iter, net::ToSocketAddrs, rc::Rc, sync::Arc, time::Duration};

use sha1::{Digest, Sha1};
use tokio::{net::UdpSocket, spawn, sync::RwLock};
use torrent_rs::{
    data::{
        metainfo::Metainfo,
        tracker::{TrackerRequest, TrackingEvent},
    },
    net::{UdpConnection, UdpManager},
    peer::{peer_handle_main, spin_peer_server, test_peer_connection, PeerConnections, PeerList},
    tracker::client::TrackerClient,
    util::{ApplyTransform, IntoHexString},
    TorrentContext,
};

const TEST_TORRENT_FILE: &'static str = "the-northman.torrent";
const PEER_ID: &'static str = "123456789--qweasdzxc";
const TRACKER_URL: &'static str = "http://thetracker.org/announce";

#[tokio::main]
async fn main() {
    let metainfo = Metainfo::from_torrent_file(TEST_TORRENT_FILE).unwrap();
    let info_hash = serde_bencode::to_bytes(&metainfo.info).unwrap();

    let mut sha1_hasher = Sha1::new();
    sha1_hasher.update(&info_hash);
    let info_hash: [u8; 20] = sha1_hasher.finalize().into();

    let context = Arc::new(TorrentContext {
        self_peer_id: Arc::new(array::from_fn(|i| i as u8)),
        info_hash: Arc::new(info_hash),
        piece_hashes: metainfo.info.get_pieces_as_sha1_hex().into(),
    });
    // let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:8080").await.unwrap());
    // let udp_connection = UdpConnection::new(UdpManager::new("0.0.0.0:8080").await.unwrap());

    let trackers = metainfo
        .announce_list
        .into_iter()
        .flatten()
        .flatten()
        .chain(iter::once(metainfo.announce.clone()))
        .collect::<Vec<_>>();
    dbg!(&trackers);

    let peer_addrs = Arc::new(std::sync::Mutex::new(vec!["127.0.0.1:8080".to_socket_addrs().unwrap().next().unwrap()]));
    let peer_list = Arc::new(tokio::sync::RwLock::new(PeerList::new()));

    let server_listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let server = tokio::spawn(
        spin_peer_server(
            context.clone(),
            peer_addrs.clone(),
            server_listener,
        )
    );

    let client = tokio::spawn(
        peer_handle_main(
            context.clone(),
            peer_addrs.clone(),
            peer_list.clone(),
            PeerConnections::new(),
        )
    );

    client.await.unwrap();
    server.await.unwrap();

    // let peers = Arc::new(std::sync::RwLock::new(Vec::new()));

    // let context_move = context.clone();
    // let peers_move = peers.clone();
    // spawn(async move {
    //     let mut tracker_client =
    //         TrackerClient::with_trackers(context_move, peers_move, "0.0.0.0:8080", &trackers).await;
    
    //     tracker_client.start().await;
    // });

    // loop {
    //     tokio::time::sleep(Duration::from_millis(5 * 1000)).await;
    //     let ps_read = peers.read().unwrap();
    //     let ps = ps_read.as_slice();
    //     println!("Peers: {:?}", ps);
    //     let mut do_break = false;
    //     for peer in ps {
    //         match test_peer_connection(peer.clone(), context.clone()).await {
    //             Ok(()) => {},
    //             Err(e) => { dbg!(e); },
    //         }
    //         do_break = true;
    //     }
    //     if do_break {
    //         break;
    //     }
    // }

    // test_peer_connection("184.75.223.227:47931".to_socket_addrs().unwrap().next().unwrap(), context.clone()).await;

    // let tracker_clients = trackers
    //     .iter()
    //     .map(|tracker| {
    //         Arc::new(TrackerClient::new(
    //             context.clone(),
    //             udp_connection.clone(),
    //             tracker,
    //         ))
    //     })
    //     .collect::<Vec<_>>();

    // let mut tracker_responses = Vec::new();
    // {
    //     let mut handles = Vec::new();
    //     for client in &tracker_clients {
    //         let client = client.clone();
    //         let handle = spawn(async move {
    //             let client = client;
    //             client.send_udp().await
    //         });
    //         handles.push(handle);
    //     }

    //     for handle in handles {
    //         tracker_responses.push(handle.await);
    //     }
    // }
    // dbg!(&tracker_responses);

    // let peer = tracker_result.peers.0.get(0).unwrap();
    // let peer_client = PeerClient::new(
    //     peer.ip_address,
    //     peer.port,
    //     Rc::new(info_hash),
    //     Rc::new(array::from_fn(|_i| rand::random())),
    // );
    // let res = peer_client.handshake().await;
    // let _ = dbg!(res);
}
