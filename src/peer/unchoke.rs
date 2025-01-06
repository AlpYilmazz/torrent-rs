
use rand::{thread_rng, RngCore};

use crate::*;
use super::*;

const RUN_UNCHOKE_PER_MILLISECONDS: u64 = 15 * 1000;
pub async fn spin_unchoke_strategy(torrent_id: TorrentId, peer_list: Global<PeerList>) {
    loop {
        println!("Unchoke sweep started.");

        let mut peer_list_write = peer_list.write().await;

        let peer_count = peer_list_write.peers.len();
        let mut unchoked = [(false, 0); MAX_UNCHOKED_COUNT + 1];
        for unchoked_i in 0..MAX_UNCHOKED_COUNT {
            let mut index = -1;
            let mut max_upload = 0;
            for (i, peer) in peer_list_write.peers.iter().enumerate() {
                let mut peer_write = peer.write().await;
                if index == -1 || peer_write.stats.upload > max_upload {
                    index = i as i32;
                    max_upload = peer_write.stats.upload;
                }
                peer_write.stats = PeerStats::init();
            }
            if index == -1 {
                break;
            } else {
                unchoked[unchoked_i] = (true, index as usize);
            }
        }

        if peer_count > MAX_UNCHOKED_COUNT {
            let mut rng = thread_rng();
            let mut rand_peer_index = (rng.next_u64() % peer_count as u64) as usize;
            {
                let mut unchoked_iter = unchoked.iter().filter(|(occupied, _)| *occupied);
                while let Some(_) = unchoked_iter.position(|(_, index)| *index == rand_peer_index) {
                    rand_peer_index = (rng.next_u64() % peer_count as u64) as usize;
                }
            }
            unchoked[MAX_UNCHOKED_COUNT] = (true, rand_peer_index);
        }

        peer_list_write.unchoked.clear();
        peer_list_write.unchoked.extend(
            unchoked
                .iter()
                .filter(|(occupied, _)| *occupied)
                .map(|(_, index)| *index),
        );

        println!(
            "torrent [{}] -> Unchoked: [{:?}]",
            torrent_id, &peer_list_write.unchoked
        );

        println!("Unchoke sweep ended.");

        tokio::time::sleep(std::time::Duration::from_millis(
            RUN_UNCHOKE_PER_MILLISECONDS,
        ))
        .await;
    }
}