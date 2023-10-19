use std::{collections::HashMap, io, net::SocketAddr, ops::Deref, sync::Arc};

use tokio::{
    net::{TcpListener, UdpSocket},
    sync::RwLock,
};

const BUFFER_SIZE: usize = 1000;
type Buffer = [u8; BUFFER_SIZE];

pub enum ConnectionState {
    Idle,
    WaitingResponse,
    Received,
}

#[derive(Clone)]
pub struct UdpConnection(Arc<RwLock<UdpManager>>);

impl Deref for UdpConnection {
    type Target = RwLock<UdpManager>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UdpConnection {
    pub fn new(udp_manager: UdpManager) -> Self {
        Self(Arc::new(RwLock::new(udp_manager)))
    }

    pub async fn recv_periodically(connection: UdpConnection) {
        let mut buf = [0; BUFFER_SIZE];
        loop {
            let mut received = false;
            let mut received_n_bytes = 0;
            let mut received_from_host = None;
            {
                let udp_manager = connection.read().await;
                match udp_manager.sock.recv_from(&mut buf).await {
                    Ok((n_bytes, from_host)) => {
                        received = true;
                        received_n_bytes = n_bytes;
                        received_from_host = Some(from_host);
                    }
                    Err(e) => {
                        println!("Tracker Network Error: {:?}", e);
                    }
                }
            }
            if received {
                let mut udp_manager = connection.write().await;
                udp_manager.state.get_mut(received_from_host.as_ref().unwrap()).map(|e| {
                    e.state = ConnectionState::Received;
                    e.n_bytes = received_n_bytes;
                    e.buffer[..received_n_bytes].copy_from_slice(&buf[..received_n_bytes]);
                });
            }
        }
    }
}

struct CommunicationState {
    state: ConnectionState,
    n_bytes: usize,
    buffer: Buffer,
}

impl Default for CommunicationState {
    fn default() -> Self {
        Self {
            state: ConnectionState::Idle,
            n_bytes: 0,
            buffer: [0; BUFFER_SIZE],
        }
    }
}

pub struct UdpManager {
    sock: UdpSocket,
    state: HashMap<SocketAddr, CommunicationState>,
}

impl UdpManager {
    pub async fn new(bind_addr: &str) -> io::Result<Self> {
        Ok(Self {
            sock: UdpSocket::bind(bind_addr).await?,
            state: HashMap::new(),
        })
    }

    pub fn connect(&mut self, target: SocketAddr) {
        self.state.insert(target, CommunicationState::default());
    }

    pub fn disconnect(&mut self, target: SocketAddr) {
        self.state.remove(&target);
    }

    pub fn get_buffer(&self, host: &SocketAddr) -> Option<&Buffer> {
        self.state.get(host).map(|state| &state.buffer)
    }
}

pub struct TcpManager {
    listener: Arc<TcpListener>,
}

impl TcpManager {
    pub async fn new(bind_addr: &str) -> io::Result<Self> {
        Ok(Self {
            listener: Arc::new(TcpListener::bind(bind_addr).await?),
        })
    }

    pub fn get_listener(&self) -> Arc<TcpListener> {
        self.listener.clone()
    }
}
