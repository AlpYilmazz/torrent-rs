use bytepack::{
    base::{ByteSize, ConstByteSize, SplatDrain, Throw},
    pack::BytePack,
    unpack::ByteUnpack,
};
use bytepack_proc_macro::{BytePack, ByteSize, ByteUnpack, ConstByteSize};

#[derive(Debug, ConstByteSize, ByteSize, BytePack, ByteUnpack)]
pub struct Handshake {
    pub protocol_len: u8,       // char: 19
    pub protocol: [u8; 19],     // 'BitTorrent protocol'
    pub reserved: Throw<u8, 8>, // reserved for extensions
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub const fn const_hold_size() -> usize {
        100
    }

    pub fn new(info_hash: &[u8; 20], peer_id: &[u8; 20]) -> Self {
        Self {
            protocol_len: 19,
            protocol: "BitTorrent protocol".as_bytes().try_into().unwrap(),
            reserved: Throw::new(),
            info_hash: info_hash.clone(),
            peer_id: peer_id.clone(),
        }
    }
}

pub const PEER_MESSAGE_CHOKE: u8 = 0;
pub const PEER_MESSAGE_UNCHOKE: u8 = 1;
pub const PEER_MESSAGE_INTERESTED: u8 = 2;
pub const PEER_MESSAGE_NOTINTERESTED: u8 = 3;
pub const PEER_MESSAGE_HAVE: u8 = 4;
pub const PEER_MESSAGE_BITFIELD: u8 = 5;
pub const PEER_MESSAGE_REQUEST: u8 = 6;
pub const PEER_MESSAGE_PIECE: u8 = 7;
pub const PEER_MESSAGE_CANCEL: u8 = 8;

#[derive(Debug)]
pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(Have),
    Bitfield(Bitfield),
    Request(Request),
    Piece(Piece),
    Cancel(Cancel),
}

impl PeerMessage {
    pub fn message_type(&self) -> u8 {
        match self {
            Self::Choke => PEER_MESSAGE_CHOKE,
            Self::Unchoke => PEER_MESSAGE_UNCHOKE,
            Self::Interested => PEER_MESSAGE_INTERESTED,
            Self::NotInterested => PEER_MESSAGE_NOTINTERESTED,
            Self::Have(_) => PEER_MESSAGE_HAVE,
            Self::Bitfield(_) => PEER_MESSAGE_BITFIELD,
            Self::Request(_) => PEER_MESSAGE_REQUEST,
            Self::Piece(_) => PEER_MESSAGE_PIECE,
            Self::Cancel(_) => PEER_MESSAGE_CANCEL,
        }
    }

    // pub fn read_from(mtype: u8, buf: &[u8]) -> Result<Self, ()> {
    //     // let mtype = u8::unpack(buf)?;
    //     // let buf = &buf[u8::const_byte_size()..];
    //     match mtype {
    //         PEER_MESSAGE_CHOKE          => Ok(PeerMessage::Choke),
    //         PEER_MESSAGE_UNCHOKE        => Ok(PeerMessage::Unchoke),
    //         PEER_MESSAGE_INTERESTED     => Ok(PeerMessage::Interested),
    //         PEER_MESSAGE_NOTINTERESTED  => Ok(PeerMessage::NotInterested),
    //         PEER_MESSAGE_HAVE           => Ok(PeerMessage::Have(Have::unpack(buf)?)),
    //         PEER_MESSAGE_BITFIELD       => Ok(PeerMessage::Bitfield(Bitfield::unpack(buf)?)),
    //         PEER_MESSAGE_REQUEST        => Ok(PeerMessage::Request(Request::unpack(buf)?)),
    //         PEER_MESSAGE_PIECE          => Ok(PeerMessage::Piece(Piece::unpack(buf)?)),
    //         PEER_MESSAGE_CANCEL         => Ok(PeerMessage::Cancel(Cancel::unpack(buf)?)),
    //         _ => Err(()),
    //     }
    // }
}

impl ByteSize for PeerMessage {
    fn byte_size(&self) -> usize {
        u8::const_byte_size()
            + match self {
                Self::Choke => 0,
                Self::Unchoke => 0,
                Self::Interested => 0,
                Self::NotInterested => 0,
                Self::Have(_) => Have::const_byte_size(),
                Self::Bitfield(b) => b.byte_size(),
                Self::Request(_) => Request::const_byte_size(),
                Self::Piece(p) => p.byte_size(),
                Self::Cancel(_) => Cancel::const_byte_size(),
            }
    }
}

impl BytePack for PeerMessage {
    fn pack(&self, buf: &mut [u8]) -> Result<(), ()> {
        self.message_type().pack(buf)?;
        let buf = &mut buf[u8::const_byte_size()..0];
        match self {
            Self::Have(m) => m.pack(buf),
            Self::Bitfield(m) => m.pack(buf),
            Self::Request(m) => m.pack(buf),
            Self::Piece(m) => m.pack(buf),
            Self::Cancel(m) => m.pack(buf),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, ConstByteSize, ByteSize, BytePack, ByteUnpack)]
pub struct Have {
    pub index: u32,
}

#[derive(Debug, ByteSize, BytePack, ByteUnpack)]
pub struct Bitfield {
    pub bitfield: SplatDrain<u8>,
}

#[derive(Debug, ConstByteSize, ByteSize, BytePack, ByteUnpack)]
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

#[derive(Debug, ConstByteSize, ByteSize, BytePack, ByteUnpack)]
pub struct Cancel {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}
