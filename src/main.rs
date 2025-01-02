use std::{
    iter,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use sha1::{Digest, Sha1};
use tokio::sync::mpsc;
use torrent_rs::{
    data::metainfo::{FileMode, Metainfo},
    make_global,
    peer::peer_handle_main,
    fileio::TorrentFile,
    Global, InfoHash, PeerId, SingleTorrent, TorrentCollection, TorrentContext, TorrentId,
};

const TEST_TORRENT_FILE: &'static str = "res/godel.torrent";
// const TEST_TORRENT_FILE: &'static str = "res/the-northman.torrent";
const PEER_ID: &'static str = "123456789--qweasdzxc";
// const TRACKER_URL: &'static str = "http://thetracker.org/announce";

#[tokio::main]
async fn main() {
    let metainfo = Metainfo::from_torrent_file(TEST_TORRENT_FILE).unwrap();
    let info_bencoded = serde_bencode::to_bytes(&metainfo.info).unwrap();

    let mut sha1_hasher = Sha1::new();
    sha1_hasher.update(&info_bencoded);
    let info_hash: [u8; 20] = sha1_hasher.finalize().into();

    let piece_hashes = metainfo.info.get_pieces_as_sha1_hex();
    let piece_hashes_count = piece_hashes.len();

    let torrent_id = 0;
    let context = make_global!(TorrentContext {
        self_peer_id: PeerId::from_raw(PEER_ID.as_bytes().try_into().unwrap()),
        torrents: vec![SingleTorrent {
            id: torrent_id,
            info_hash: InfoHash::from_raw(info_hash),
            piece_length: metainfo.info.piece_length,
            piece_count: piece_hashes_count,
            piece_hashes: piece_hashes.into(),
        }],
    });

    let FileMode::SingleFile(single_file) = metainfo.info.mode else {
        println!("MultipleFiles torrents are not yet supported.");
        return;
    };
    let file_length = single_file.length;

    let path = metainfo.info.name;
    let piece_count = piece_hashes_count as u32;
    let piece_length = metainfo.info.piece_length;
    // let end_piece_length = (file_length - ((piece_count - 1) as u64 * piece_length as u64)) as u32;

    let torrent_collection: Global<TorrentCollection> = make_global!([(
        torrent_id,
        make_global!(TorrentFile::new(
            &path,
            file_length,
            piece_count as usize,
            piece_length as usize,
        )
        .await
        .unwrap()),
    )]
    .into_iter()
    .collect());

    let trackers = metainfo
        .announce_list
        .into_iter()
        .flatten()
        .flatten()
        .chain(iter::once(metainfo.announce.clone()))
        .collect::<Vec<_>>();
    dbg!(&trackers);

    let (peer_addrs_channel_sender, peer_addrs_channel_receiver) =
        mpsc::channel::<(TorrentId, Arc<[SocketAddr]>)>(100);

    let client = tokio::spawn(peer_handle_main(
        context.clone(),
        torrent_collection,
        peer_addrs_channel_receiver,
    ));

    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
    while let Ok(()) = peer_addrs_channel_sender
        .send((
            torrent_id,
            Arc::new(["127.0.0.1:8080".to_socket_addrs().unwrap().next().unwrap()]),
        ))
        .await
    {}

    client.await.unwrap();

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
