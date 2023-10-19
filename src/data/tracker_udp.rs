use std::{rc::Rc, sync::Arc};

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

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct ActionSwitch {
    pub action: u32,
}

impl ActionSwitch {
    pub fn try_get_response<T: ByteUnpack + TransactionMessage>(
        &self,
        buf: &[u8],
        request_action: u32,
        transaction_id: u32,
    ) -> Result<T, Result<TrackerError, String>> {
        let action_switch = ActionSwitch::unpack(&buf).unwrap();
        let response = match action_switch.action {
            a if a == request_action => Ok(T::unpack(&buf).unwrap()),
            action::ERROR => {
                let tracker_error = TrackerError::unpack(&buf).unwrap();
                if tracker_error.transaction_id != transaction_id {
                    return Err(Err("Error transaction_id does not match".to_string()));
                }
                Err(tracker_error)
            }
            _ => return Err(Err("Unexpected action".to_string())),
        };
        let response = response.unwrap();

        if response.get_transaction_id() != transaction_id {
            return Err(Err("Response transactionid does not match".to_string()));
        }

        Ok(response)
    }
}

#[derive(Debug, ByteSize, BytePack)]
pub struct ConnectRequest {
    pub protocol_id: u64,
    pub action: u32, // 0: action::CONNECT
    pub transaction_id: u32,
}

impl ConnectRequest {
    pub const PROTOCOL_ID: u64 = 0x41727101980;
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct ConnectResponse {
    pub action: u32, // 0: action::CONNECT
    pub transaction_id: u32,
    pub connection_id: u64,
}

#[derive(Debug, ByteSize, BytePack)]
pub struct AnnounceRequest {
    pub connection_id: u64,
    pub action: u32, // 1: action::ANNOUNCE
    pub transaction_id: u32,
    pub info_hash: Arc<[u8; 20]>, // 20 bytes
    pub peer_id: Arc<[u8; 20]>,    // 20 bytes
    pub downloaded: u64,
    pub left: u64,
    pub uploaded: u64,
    pub event: u32, // event::XXX -> 0: none (default); 1: completed; 2: started; 3: stopped
    pub ip_address: u32, // 0: default
    pub key: u32,
    pub num_want: i32, // -1: default
    pub port: u16,
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct Ipv4AnnounceResponse {
    pub action: u32, // 1: action::ANNOUNCE
    pub transaction_id: u32,
    pub interval: u32,
    pub leechers: u32,
    pub seeders: u32,
    pub peers: DrainVec<Ipv4Peer>,
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct Ipv4Peer {
    pub ip_address: u32,
    pub port: u16,
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct Ipv6AnnounceResponse {
    pub action: u32, // 1: action::ANNOUNCE
    pub transaction_id: u32,
    pub interval: u32,
    pub leechers: u32,
    pub seeders: u32,
    pub peers: DrainVec<Ipv6Peer>,
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct Ipv6Peer {
    pub ip_address: u128,
    pub port: u16,
}

#[derive(Debug, ByteSize, BytePack)]
pub struct ScrapeRequest {
    pub connection_id: u64,
    pub action: u32, // 2: action::SCRAPE
    pub transaction_id: u32,
    pub info_hashes: SplatVec<[u8; 20]>, // 20 bytes
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct ScrapeResponse {
    pub action: u32, // 2: action::SCRAPE
    pub transaction_id: u32,
    pub scrapes: DrainVec<Scrape>,
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct Scrape {
    pub seeders: u32,
    pub completed: u32,
    pub leechers: u32,
}

#[derive(Debug, ByteSize, ByteUnpack)]
pub struct TrackerError {
    pub action: u32, // 3: action::ERROR
    pub transaction_id: u32,
    pub message: DrainVec<u8>, // String
}

pub trait TransactionMessage {
    fn get_transaction_id(&self) -> u32;
}

impl TransactionMessage for ConnectRequest {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for ConnectResponse {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for AnnounceRequest {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for Ipv4AnnounceResponse {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for Ipv6AnnounceResponse {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for ScrapeRequest {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for ScrapeResponse {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}

impl TransactionMessage for TrackerError {
    fn get_transaction_id(&self) -> u32 {
        self.transaction_id
    }
}
