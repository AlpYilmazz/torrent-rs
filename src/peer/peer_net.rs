use anyhow::bail;
use bytepack::{
    base::{ByteSize, ConstByteSize},
    pack::BytePack,
    unpack::ByteUnpack,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

use super::*;
use crate::*;

pub async fn spin_send_peer_message(
    peer: Global<Peer>,
    mut message_channel: Receiver<SendPeerMessage>,
    mut write_stream: OwnedWriteHalf,
    mut net_buffer: Box<[u8]>,
) {
    loop {
        let Some(send_peer_message) = message_channel.recv().await else {
            break; // channel closed
        };

        println!(
            "Sending to [peer_id: {}]: {:?}\n",
            send_peer_message.peer_id.as_str(),
            send_peer_message.message.message_type()
        );

        let message = send_peer_message.message;
        let message_size = message.byte_size();
        message.pack(&mut net_buffer).unwrap();

        write_stream
            .write_all(&net_buffer[..message_size])
            .await
            .unwrap();
        println!("Sent message: {}", message.message_type());
    }
}

pub async fn spin_receive_peer_message(
    peer: Global<Peer>,
    message_channel: Sender<ReceivedPeerMessage>,
    mut read_stream: OwnedReadHalf,
    mut net_buffer: Box<[u8]>,
) -> anyhow::Result<()> {
    loop {
        read_stream.read_exact(&mut net_buffer[..1]).await?;
        let msg_type = net_buffer[0];

        let net_buffer = &mut net_buffer[1..];

        let peer = peer.read().await;
        if peer.terminate {
            break;
        }

        let peer_msg = match msg_type {
            PEER_MESSAGE_CHOKE => PeerMessage::Choke,
            PEER_MESSAGE_UNCHOKE => PeerMessage::Unchoke,
            PEER_MESSAGE_INTERESTED => PeerMessage::Interested,
            PEER_MESSAGE_NOTINTERESTED => PeerMessage::NotInterested,
            PEER_MESSAGE_HAVE => {
                let have_len = peer_message::Have::const_byte_size();
                read_stream.read_exact(&mut net_buffer[..have_len]).await?;

                let Ok(pm_have) = peer_message::Have::unpack(net_buffer) else {
                    bail!("Faulty peer message [Have]");
                };
                PeerMessage::Have(pm_have)
            }
            PEER_MESSAGE_BITFIELD => {
                let bitfield_len = bitcount_to_bytecount(peer.piece_count);
                read_stream
                    .read_exact(&mut net_buffer[..bitfield_len])
                    .await?;

                let Ok(pm_bitfield) = peer_message::Bitfield::unpack(net_buffer) else {
                    bail!("Faulty peer message [Bitfield]");
                };
                PeerMessage::Bitfield(pm_bitfield)
            }
            PEER_MESSAGE_REQUEST => {
                let request_len = peer_message::Request::const_byte_size();
                read_stream
                    .read_exact(&mut net_buffer[..request_len])
                    .await?;

                let Ok(pm_request) = peer_message::Request::unpack(net_buffer) else {
                    bail!("Faulty peer message [Request]");
                };
                PeerMessage::Request(pm_request)
            }
            PEER_MESSAGE_PIECE => {
                // TODO: can wait for multiple pieces, piece requests could be buffered
                let Some(piece_len) = peer.state.waiting_on.as_ref().map(|w| w.length) else {
                    bail!("Not expecting a piece to arrive");
                };
                let piece_len = u32::const_byte_size()      // piece_message.index
                    + u32::const_byte_size()    // piece_message.begin
                    + piece_len as usize; // piece_message.piece
                println!("piece_len: {piece_len}");

                read_stream.read_exact(&mut net_buffer[..piece_len]).await?;

                let Ok(pm_piece) = peer_message::Piece::unpack(&net_buffer[..piece_len]) else {
                    bail!("Faulty peer message [Piece]");
                };
                PeerMessage::Piece(pm_piece)
            }
            PEER_MESSAGE_CANCEL => {
                let cancel_len = peer_message::Cancel::const_byte_size();
                read_stream
                    .read_exact(&mut net_buffer[..cancel_len])
                    .await?;

                let Ok(pm_cancel) = peer_message::Cancel::unpack(net_buffer) else {
                    bail!("Faulty peer message [Cancel]");
                };
                PeerMessage::Cancel(pm_cancel)
            }
            _ => bail!("Faulty peer message [msg_type]"),
        };

        message_channel
            .send(ReceivedPeerMessage {
                torrent_id: peer.torrent_id,
                peer_index: peer.index,
                peer_id: peer.peer_id,
                message: peer_msg,
            })
            .await?;
    }

    Ok(())
}
