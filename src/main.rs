use sha1::{Digest, Sha1};
use torrent_rs::{
    data::{
        metainfo::Metainfo,
        tracker::{TrackerRequest, TrackingEvent},
    },
    tracker::client::TrackerClient,
    util::{ApplyTransform, IntoHexString},
};

const TEST_TORRENT_FILE: &'static str = "godel.torrent";
const PEER_ID: &'static str = "123456789--qweasdzxc";
const TRACKER_URL: &'static str = "http://thetracker.org/announce";

#[tokio::main]
async fn main() {
    let metainfo = Metainfo::from_torrent_file(TEST_TORRENT_FILE).unwrap();
    let tracker_client = TrackerClient::new(&metainfo.announce);

    let info = &metainfo.info;
    let info_hash = info.apply(serde_bencode::to_bytes).unwrap();

    let mut sha1_hasher = Sha1::new();
    sha1_hasher.update(&info_hash);
    let info_hash: [u8; 20] = sha1_hasher.finalize().into();

    let tracker_request = TrackerRequest {
        info_hash: info_hash.as_ref().into_hex_string(),
        peer_id: PEER_ID.to_string(),
        ip: None,
        port: 6881,
        uploaded: 0,
        downloaded: 0,
        left: 100,
        event: TrackingEvent::Started,
        compact: Some(false),
    };
    dbg!(&tracker_request);

    let tracker_result = tracker_client.send_udp(&tracker_request).await;
    let _ = dbg!(tracker_result);
}
