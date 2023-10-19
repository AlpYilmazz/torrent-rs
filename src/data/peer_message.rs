use std::rc::Rc;

use bytepack::{
    base::{ByteSize, SplatDrain, Throw},
    pack::BytePack,
    unpack::ByteUnpack,
};
use bytepack_proc_macro::{BytePack, ByteSize, ByteUnpack};

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Handshake {
    pub protocol_len: u8,        // char: 19
    pub protocol: Box<[u8; 19]>, // 'BitTorrent protocol'
    pub reserved: Throw<u8, 8>,  // reserved for extensions
    pub info_hash: Rc<[u8; 20]>,
    pub peer_id: Rc<[u8; 20]>,
}

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Choke;

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Unchoke;

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Interested;

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct NotInterested;

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Have {
    pub index: u32,
}

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Bitfield {
    pub bitfield: SplatDrain<u8>,
}

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Request {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Piece {
    pub index: u32,
    pub begin: u32,
    pub piece: SplatDrain<u8>,
}

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Cancel {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}
