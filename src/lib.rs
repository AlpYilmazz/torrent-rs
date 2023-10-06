pub mod data;
pub mod tracker;
pub mod util;

#[cfg(test)]
mod tests {
    use serde_bytes::ByteBuf;
    use sha1::{Sha1, Digest};

    use crate::{
        data::{
            metainfo::{FileMode, Metainfo},
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
        let info_hash = info
            .apply(serde_bencode::to_bytes)
            .unwrap();
        
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

        /*
        // let pieces = String::from_utf8(
        //     metainfo.info.pieces.clone().into_vec().into_iter().take(20).collect::<Vec<_>>()
        // ).unwrap();

        let FileMode::SingleFile { length, md5sum } = &metainfo.info.mode else {
            Result::<(), ()>::Err(()).unwrap();
            return;
        };
        let num_pieces = (length / metainfo.info.piece_length)
            + (length % metainfo.info.piece_length > 0) as u64;

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
        */
    }
}
