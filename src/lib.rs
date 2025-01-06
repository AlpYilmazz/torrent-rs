use core::str;
use std::{collections::HashMap, sync::Arc};

use fileio::TorrentFile;

pub mod data;
pub mod fileio;
pub mod peer;
pub mod tracker;
pub mod util;

pub type Global<T> = std::sync::Arc<tokio::sync::RwLock<T>>;

#[macro_export]
macro_rules! make_global {
    ($expr: expr) => {
        std::sync::Arc::new(tokio::sync::RwLock::new($expr))
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId {
    pub id: [u8; 20],
}

impl PeerId {
    pub fn from_raw(id: [u8; 20]) -> Self {
        Self { id }
    }

    pub fn uninit() -> Self {
        Self::from_raw([0; 20])
    }

    pub fn is_zeroed(&self) -> bool {
        self.id.iter().all(|e| *e == 0)
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.id) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InfoHash {
    pub hash: [u8; 20],
}

impl InfoHash {
    pub fn from_raw(hash: [u8; 20]) -> Self {
        Self { hash }
    }
}

pub type TorrentId = usize;

pub struct TorrentContext {
    pub self_peer_id: PeerId,
    pub torrents: Vec<Arc<SingleTorrent>>,
}

impl TorrentContext {
    pub fn new(self_peer_id: &str) -> Self {
        Self {
            self_peer_id: PeerId::from_raw(self_peer_id.as_bytes().try_into().unwrap()),
            torrents: Vec::new(),
        }
    }
}

pub struct SingleTorrent {
    pub id: TorrentId,
    pub info_hash: InfoHash,
    pub piece_length: u32,
    pub piece_count: usize,
    pub piece_hashes: Arc<[String]>,
}

pub type TorrentCollection = HashMap<TorrentId, Global<TorrentFile>>;

#[cfg(test)]
mod tests {
    use serde_bytes::ByteBuf;
    use sha1::{Digest, Sha1};

    use crate::{
        data::{
            metainfo::{FileMode, Metainfo, SingleFile},
            tracker::{TrackerRequest, TrackingEvent},
        },
        util::{ApplyTransform, IntoHexString},
    };

    const TEST_TORRENT_FILE: &'static str = "godel.torrent";
    const PEER_ID: &'static str = "123456789--qweasdzxc";

    #[test]
    fn test_main() {
        let mut metainfo = Metainfo::from_torrent_file(TEST_TORRENT_FILE).unwrap();

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
        dbg!(tracker_request);

        // let pieces = String::from_utf8(
        //     metainfo.info.pieces.clone().into_vec().into_iter().take(20).collect::<Vec<_>>()
        // ).unwrap();

        let FileMode::SingleFile(SingleFile { length, .. }) = &metainfo.info.mode else {
            Result::<(), ()>::Err(()).unwrap();
            return;
        };
        let num_pieces = (length / metainfo.info.piece_length as u64)
            + (length % metainfo.info.piece_length as u64 > 0) as u64;

        let pieces_bytes_len = metainfo.info.pieces.len();
        let pieces = metainfo.info.get_pieces_as_sha1_hex();
        let pieces_len = pieces.len();

        metainfo.info.pieces = ByteBuf::from(
            metainfo
                .info
                .pieces
                .into_iter()
                .take(4)
                .collect::<Vec<u8>>(),
        );

        dbg!(metainfo);
        // dbg!(pieces);
        dbg!(pieces_bytes_len);
        dbg!(pieces_len);
        dbg!(num_pieces);
    }
}
