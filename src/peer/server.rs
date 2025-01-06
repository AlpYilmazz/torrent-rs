use std::net::SocketAddr;

use anyhow::bail;
use bytepack::{base::{ByteSize, ConstByteSize}, pack::BytePack, unpack::ByteUnpack};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};

use crate::{data::peer_message::{self, Handshake, PeerMessage}, util::{create_buffer, BitVecU8Ext}, Global, PeerId, TorrentCollection, TorrentContext, TorrentId};

use super::{PeerAddEvent, PeerAddEventChannels};


pub struct PeerConnectionInit {
    pub torrent_id: TorrentId,
    pub peer_id: PeerId,
}

pub async fn handle_peer_handshake(
    context: Global<TorrentContext>,
    torrent_collection: Global<TorrentCollection>,
    peer_addr: SocketAddr,
    stream: &mut TcpStream,
) -> anyhow::Result<PeerConnectionInit> {
    let mut net_buffer = [0; Handshake::const_hold_size()];

    stream
        .read_exact(&mut net_buffer[..Handshake::const_byte_size()])
        .await?;
    let Ok(handshake_request) = Handshake::unpack(net_buffer.as_ref()) else {
        bail!("Handshake could not be parsed.");
    };

    println!("Server received from [peer_addr: {:?}]", peer_addr);
    // dbg!(&handshake_request);

    let (torrent_id, self_peer_id) = {
        let context_read = context.read().await;
        let Some(torrent_id) = context_read
            .torrents
            .iter()
            .position(|st| st.info_hash.hash == handshake_request.info_hash)
        else {
            bail!("Requested Torrent doesnt exist on self peer");
        };
        (torrent_id, context_read.self_peer_id.clone())
    };

    let handshake_response = Handshake::new(&handshake_request.info_hash, &self_peer_id.id);
    handshake_response.pack(net_buffer.as_mut()).unwrap();
    stream
        .write_all(&net_buffer[..Handshake::const_byte_size()])
        .await?;

    let bitfield = {
        let torrent_collection_read = torrent_collection.read().await;
        let torrent_file_read = torrent_collection_read
            .get(&torrent_id)
            .unwrap()
            .read()
            .await;
        torrent_file_read.bitfield.into_u8_vec()
    };

    let bitfield_message = PeerMessage::Bitfield(peer_message::Bitfield {
        bitfield: bytepack::base::SplatDrain::Splat(bitfield),
    });

    let mut buffer = create_buffer(bitfield_message.byte_size());
    bitfield_message.pack(&mut buffer).unwrap();
    stream.write_all(&buffer).await?;

    Ok(PeerConnectionInit {
        torrent_id,
        peer_id: PeerId::from_raw(handshake_response.peer_id),
    })
}

pub async fn spin_peer_server(
    torrent_context: Global<TorrentContext>,
    torrent_collection: Global<TorrentCollection>,
    peer_add_channels: Global<PeerAddEventChannels>,
    listener: TcpListener,
) {
    loop {
        let (stream, peer_addr) = listener.accept().await.unwrap();

        let torrent_context_clone = torrent_context.clone();
        let torrent_collection_clone = torrent_collection.clone();
        let peer_add_channels_clone = peer_add_channels.clone();

        tokio::spawn(async move {
            let mut stream = stream;

            let handshake_result = handle_peer_handshake(
                torrent_context_clone.clone(),
                torrent_collection_clone,
                peer_addr,
                &mut stream,
            )
            .await;

            let PeerConnectionInit {
                torrent_id,
                peer_id,
            } = match handshake_result {
                Ok(peer_connection_init) => peer_connection_init,
                Err(err) => {
                    dbg!(err);
                    stream.shutdown().await.unwrap();
                    return;
                }
            };

            let peer_add_channel = {
                let peer_add_channels_read = peer_add_channels_clone.read().await;
                peer_add_channels_read.get(&torrent_id).unwrap().clone()
            };

            let Ok(_) = peer_add_channel
                .send(PeerAddEvent::Connected(peer_id, peer_addr, stream))
                .await
            else {
                return; // channel closed
            };
        });
    }
}