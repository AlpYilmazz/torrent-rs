use std::{
    cell::RefCell,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    rc::Rc,
};

use bytepack::{
    base::{ByteSize, Throw},
    pack::BytePack,
    unpack::ByteUnpack,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{data::peer_message::Handshake, util::UnifyError};

const BUFFER_SIZE: usize = 10000;

pub struct PeerClient {
    ip: u32,
    port: u16,
    info_hash: Rc<[u8; 20]>,
    peer_id: Rc<[u8; 20]>,
    buffer: RefCell<[u8; BUFFER_SIZE]>,
}

impl PeerClient {
    pub fn new(ip: u32, port: u16, info_hash: Rc<[u8; 20]>, peer_id: Rc<[u8; 20]>) -> Self {
        Self {
            ip,
            port,
            info_hash,
            peer_id,
            buffer: RefCell::new([0; BUFFER_SIZE]),
        }
    }

    pub async fn handshake(&self) -> Result<(), (usize, String)> {
        let ip = "165.255.115.86".parse::<Ipv4Addr>().unwrap();
        let port = 12242;
        let mut sock = TcpStream::connect(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(ip),
            port,
        )))
        .await
        .unify_error(1)?;

        let handshake_request = Handshake {
            protocol_len: 19,
            protocol: Box::new(*b"BitTorrent protocol"),
            reserved: Throw::new(),
            info_hash: self.info_hash.clone(),
            peer_id: self.peer_id.clone(),
        };
        let byte_size = handshake_request.byte_size();
        let _res = handshake_request.pack(self.buffer.borrow_mut().as_mut_slice());

        sock.write_all(&self.buffer.borrow().as_slice()[..byte_size])
            .await
            .unify_error(2)?;

        let mut begin = 0;
        loop {
            sock.readable().await.unify_error(3)?;
            match sock
                .read(&mut self.buffer.borrow_mut().as_mut_slice()[begin..])
                .await
            {
                Ok(n) if n != 0 => {
                    begin += n;
                    continue;
                }
                Ok(_n) => break,
                Err(e) => return Err(e).unify_error(4),
            }
        }
        let response_len = begin;
        println!("response_len: {}", response_len);

        let handshake_response =
            Handshake::unpack(&self.buffer.borrow().as_slice()[..response_len]).unwrap();

        dbg!(&handshake_response);

        // let mut begin = 0;
        // loop {
        //     sock.writable().await.unify_error(2)?;
        //     match sock.write(&self.buffer.borrow().as_slice()[begin..byte_size]).await {
        //         Ok(n) if begin + n < byte_size => {
        //             begin += n;
        //             continue;
        //         },
        //         Ok(_n) => break,
        //         Err(e) => return Err(e).unify_error(3),
        //     }
        // }

        Ok(())
    }
}
