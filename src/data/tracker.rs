use std::fmt;

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Debug)]
pub struct TrackerRequest {
    pub info_hash: String, // 20 bytes sha1 hex
    pub peer_id: String,   // len 20 -> 20 bytes? (20 C chars)
    pub ip: Option<String>,
    pub port: u32,       // 6881-6889
    pub uploaded: u64,   // encoded in base 10 ascii
    pub downloaded: u64, // encoded in base 10 ascii
    pub left: u64,       // encoded in base 10 ascii
    pub event: TrackingEvent,
    pub compact: Option<bool>,
}

impl TrackerRequest {
    pub fn as_query(&self) -> Vec<(&'static str, String)> {
        let mut qv = vec![
            ("info_hash", self.info_hash.clone()),
            ("peer_id", self.peer_id.clone()),
            // ip
            ("port", self.port.to_string()),
            ("uploaded", self.uploaded.to_string()),
            ("downloaded", self.downloaded.to_string()),
            ("left", self.left.to_string()),
            ("event", self.event.to_string()),
            // compact
        ];

        if let Some(ip) = &self.ip {
            qv.push(("ip", ip.clone()));
        }
        if let Some(compact) = &self.compact {
            qv.push(("compact", (*compact as u32).to_string()));
        }

        qv
    }
}

#[derive(Debug)]
pub enum TrackingEvent {
    Started,
    Completed,
    Stopped,
    Empty,
}

impl fmt::Display for TrackingEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Started => write!(f, "started"),
            Self::Completed => write!(f, "completed"),
            Self::Stopped => write!(f, "stopped"),
            Self::Empty => write!(f, "empty"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum TrackerResult {
    Failure(TrackerFailure),
    Success(TrackerResponse),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrackerFailure {
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: Peers,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Peers {
    Compact(ByteBuf),
    Original(Vec<Peer>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Peer {
    pub peer_id: String, // len 20 -> 20 bytes? (20 C chars)
    pub ip: String,
    pub port: u32,
}
