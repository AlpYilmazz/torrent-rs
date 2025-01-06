use tokio::sync::mpsc;
use torrent_rs::{data::metainfo::Metainfo, peer::peer_handle_main};

const TEST_TORRENT_FILE: &'static str = "res/godel.torrent";

#[tokio::main]
async fn main() {

    use bitvec::prelude::*;
    let mut bv = BitVec::<_, LocalBits>::from_vec(vec![1u8, 2, 19]);

    for i in 0..3*8 {
        if i != 0 && i % 8 == 0 {
            print!(" ");
        }
        let bit = *bv.get(i).unwrap() as u8;
        print!("{}", bit);
    }
    print!("\n");

    bv.clear();
    bv.extend_from_raw_slice(&[7, 8, 100]);

    for i in 0..3*8 {
        if i != 0 && i % 8 == 0 {
            print!(" ");
        }
        let bit = *bv.get(i).unwrap() as u8;
        print!("{}", bit);
    }
    print!("\n");

    let dom = bv.domain().collect::<Vec<_>>();
    dbg!(dom);

    return;

    let (add_torrent_channel_sender, add_torrent_channel_receiver) = mpsc::channel::<Metainfo>(100);

    let client = tokio::spawn(peer_handle_main(add_torrent_channel_receiver));

    println!("Adding torrent metainfo in 5 secs...");
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
    let mut metainfo = Metainfo::from_torrent_file(TEST_TORRENT_FILE).unwrap();
    metainfo.info.name = "Godel.pdf".to_string(); // TODO
    while let Err(_) = add_torrent_channel_sender.send(metainfo.clone()).await {}

    client.await.unwrap();
}
