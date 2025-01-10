use tokio::sync::mpsc;
use torrent_rs::{data::metainfo::Metainfo, peer::peer_handle_main};

const TEST_TORRENT_FILE: &'static str = "res/godel.torrent";

#[tokio::main]
async fn main() {

    use torrent_rs::cache_read;
    use torrent_rs::util::*;

    let mut cache: FixedCache<Box<[u8]>, 4> = FixedCache::new();

    cache.save(10, create_buffer(100));
    cache.save(20, create_buffer(100));
    cache.save(30, create_buffer(100));
    cache.save(40, create_buffer(100));
    cache.save(50, create_buffer(100)); // drop 0
    cache.save(60, create_buffer(100)); // drop 1
    cache.save(60, create_buffer(100)); // drop 1

    cache.save(10, create_buffer(100)); // drop 2
    cache.save(10, create_buffer(200)); // drop 2

    let item = cache_read!(cache, 10, { println!("cache miss"); create_buffer(100) });
    println!("item len: {}", item.len());

    drop(cache); // drop 0, 1, 2, 3

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
