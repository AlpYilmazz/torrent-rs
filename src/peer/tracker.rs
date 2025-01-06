use std::net::ToSocketAddrs;

use super::*;
use crate::*;

pub async fn spin_tracker_client(peer_add_channel: Sender<PeerAddEvent>) {
    println!("Sending peer in 5 secs...");
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
    let peer_addr = "127.0.0.1:8080".to_socket_addrs().unwrap().next().unwrap();
    while let Err(_) = peer_add_channel
        .send(PeerAddEvent::Init(Arc::new([peer_addr])))
        .await
    {}
}
