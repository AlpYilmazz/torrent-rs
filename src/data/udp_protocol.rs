use bytepack::{
    base::{ByteSize, DrainVec, SplatVec},
    pack::BytePack,
    unpack::ByteUnpack,
};
use bytepack_proc_macro::*;

pub mod action {
    pub const CONNECT: u32 = 0;
    pub const ANNOUNCE: u32 = 1;
    pub const SCRAPE: u32 = 2;
    pub const ERROR: u32 = 3;
}

pub mod event {
    pub const NONE: u32 = 0;
    pub const COMPLETED: u32 = 1;
    pub const STARTED: u32 = 2;
    pub const STOPPED: u32 = 3;
}

#[derive(ByteSize, BytePack)]
pub struct ConnectRequest {
    pub protocol_id: u64,
    pub action: u32, // 0: action::CONNECT
    pub transaction_id: u32,
}

impl ConnectRequest {
    pub const PROTOCOL_ID: u64 = 0x41727101980;
}

#[derive(ByteSize, ByteUnpack)]
pub struct ConnectResponse {
    pub action: u32, // 0: action::CONNECT
    pub transaction_id: u32,
    pub connection_id: u64,
}

#[derive(ByteSize, BytePack)]
pub struct AnnounceRequest {
    pub connection_id: u64,
    pub action: u32, // 1: action::ANNOUNCE
    pub transaction_id: u32,
    pub info_hash: Box<[u8; 20]>, // 20 bytes
    pub peer_id: Box<[u8; 20]>,   // 20 bytes
    pub downloaded: u64,
    pub left: u64,
    pub uploaded: u64,
    pub event: u32, // event::XXX -> 0: none (default); 1: completed; 2: started; 3: stopped
    pub ip_address: u32, // 0: default
    pub key: u32,
    pub num_want: i32, // -1: default
    pub port: u16,
}

#[derive(ByteSize, ByteUnpack)]
pub struct Ipv4AnnounceResponse {
    pub action: u32, // 1: action::ANNOUNCE
    pub transaction_id: u32,
    pub interval: u32,
    pub leechers: u32,
    pub seeders: u32,
    pub peers: DrainVec<Ipv4Peer>,
}

#[derive(ByteSize, ByteUnpack)]
pub struct Ipv4Peer {
    pub ip_address: u32,
    pub port: u16,
}

#[derive(ByteSize, ByteUnpack)]
pub struct Ipv6AnnounceResponse {
    pub action: u32, // 1: action::ANNOUNCE
    pub transaction_id: u32,
    pub interval: u32,
    pub leechers: u32,
    pub seeders: u32,
    pub peers: DrainVec<Ipv6Peer>,
}

#[derive(ByteSize, ByteUnpack)]
pub struct Ipv6Peer {
    pub ip_address: u128,
    pub port: u16,
}

#[derive(ByteSize, BytePack)]
pub struct ScrapeRequest {
    pub connection_id: u64,
    pub action: u32, // 2: action::SCRAPE
    pub transaction_id: u32,
    pub info_hashes: SplatVec<[u8; 20]>, // 20 bytes
}

#[derive(ByteSize, ByteUnpack)]
pub struct ScrapeResponse {
    pub action: u32, // 2: action::SCRAPE
    pub transaction_id: u32,
    pub scrapes: DrainVec<Scrape>,
}

#[derive(ByteSize, ByteUnpack)]
pub struct Scrape {
    pub seeders: u32,
    pub completed: u32,
    pub leechers: u32,
}

#[derive(ByteSize, ByteUnpack)]
pub struct Error {
    pub action: u32, // 3: action::ERROR
    pub transaction_id: u32,
    pub message: DrainVec<u8>, // String
}
