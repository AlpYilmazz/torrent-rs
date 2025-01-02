// use std::{
//     array,
//     borrow::BorrowMut,
//     cell::RefCell,
//     collections::HashMap,
//     net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
//     ops::Deref,
//     rc::Rc,
//     sync::Arc,
// };

// use bytepack::{base::ByteSize, pack::BytePack, unpack::ByteUnpack};
// use reqwest::{Client, RequestBuilder};
// use tokio::{net::UdpSocket, sync::RwLock};
// use url::Url;

// use crate::{
//     data::{
//         tracker::{TrackerRequest, TrackerResult},
//         tracker_udp::{
//             action, event, ActionSwitch, AnnounceRequest, ConnectRequest, ConnectResponse,
//             Ipv4AnnounceResponse, TrackerError,
//         },
//     },
//     net::{UdpConnection, UdpManager},
//     util::{ApplyTransform, UnifyError},
//     TorrentContext,
// };

// const BUFFER_SIZE: usize = 5000;

// type ConstBuffer = [u8; BUFFER_SIZE];
// type Buffer = [u8];
// type TransactionId = u32;

// #[derive(Clone, Copy)]
// enum TrackerState {
//     Init,
//     ConnectSent(TransactionId),
//     ConnectReceived,
//     AnnounceSent(TransactionId),
//     AnnounceReceived,
//     Established,
// }

// pub struct TrackerClient {
//     context: Arc<TorrentContext>,
//     // connection: UdpConnection,
//     sock: UdpSocket,
//     trackers: HashMap<SocketAddr, TrackerState>,
//     peers: Arc<std::sync::RwLock<Vec<SocketAddr>>>,
// }

// impl TrackerClient {
//     pub async fn with_trackers(
//         context: Arc<TorrentContext>,
//         peers: Arc<std::sync::RwLock<Vec<SocketAddr>>>,
//         bind: &str,
//         trackers: &[String],
//     ) -> Self {
//         Self {
//             context,
//             // connection: UdpConnection::new(UdpManager::new(bind).await.unwrap()),
//             sock: UdpSocket::bind(bind).await.unwrap(),
//             trackers: trackers
//                 .iter()
//                 .filter_map(|host| {
//                     let url = Url::parse(host).unwrap();
//                     let host = url.host_str().unwrap().to_string()
//                         + ":"
//                         + url
//                             .port()
//                             .map(|p| p.to_string())
//                             .unwrap_or_else(|| "".to_string())
//                             .as_ref();
//                     println!("Host: {}", &host);
//                     Some((
//                         host.to_socket_addrs().ok()?.next().unwrap(),
//                         TrackerState::Init,
//                     ))
//                 })
//                 .collect(),
//             peers,
//         }
//     }

//     pub async fn start(&mut self) {
//         // call Self::handle_tracker_init for each tracker
//         // receive and dispacth messages from trackers
//         // and move up the state
//         // -> Collect peers from trackers

//         // {
//         //     let mut handles = Vec::new();
//         //     for client in &tracker_clients {
//         //         let client = client.clone();
//         //         let handle = spawn(async move {
//         //             let client = client;
//         //             client.send_udp().await
//         //         });
//         //         handles.push(handle);
//         //     }

//         //     for handle in handles {
//         //         tracker_responses.push(handle.await);
//         //     }
//         // }

//         for tracker in self.trackers.keys().cloned().collect::<Vec<_>>() {
//             self.handle_tracker_init(&tracker).await;
//             println!("Handled {}", &tracker);
//         }

//         let mut buf = Box::new([0; BUFFER_SIZE]);
//         loop {
//             println!(" -- Recv -- ");
//             match self.sock.recv_from(&mut buf[..]).await {
//                 Ok((n_bytes, from_host)) => {
//                     println!("Received [{}] bytes from host: {:?}", n_bytes, &from_host);
//                     let Some(state) = self.trackers.get(&from_host).copied() else {
//                         continue;
//                     };

//                     let response_buf = &buf[..n_bytes];
//                     let action_switch = ActionSwitch::unpack(&response_buf).unwrap();

//                     match state {
//                         TrackerState::Init => {
//                             println!("Host: {:?}, Should not be Init", &from_host)
//                         }
//                         TrackerState::ConnectSent(transaction_id) => {
//                             let connect_response = action_switch.try_get_response(
//                                 &response_buf,
//                                 action::CONNECT,
//                                 transaction_id,
//                             );

//                             let connect_response: ConnectResponse = match connect_response {
//                                 Ok(r) => r,
//                                 Err(Ok(tracker_error)) => {
//                                     println!(
//                                         "Host: {:?}, Tracker Error: {:?}",
//                                         &from_host, &tracker_error.message.0
//                                     );
//                                     continue;
//                                     // return Err((
//                                     //     3,
//                                     //     String::from_utf8(tracker_error.message.0)
//                                     //         .unify_error(4)?,
//                                     // ))
//                                 }
//                                 Err(Err(e)) => {
//                                     println!("Host: {:?}, Error: {:?}", &from_host, e);
//                                     continue; // Err((5, e)),
//                                 }
//                             };
//                             self.handle_connect_response(&from_host, connect_response)
//                                 .await;
//                         }
//                         TrackerState::ConnectReceived => {}
//                         TrackerState::AnnounceSent(transaction_id) => {
//                             let announce_response = action_switch.try_get_response(
//                                 &response_buf,
//                                 action::ANNOUNCE,
//                                 transaction_id,
//                             );

//                             let announce_response: Ipv4AnnounceResponse = match announce_response {
//                                 Ok(r) => r,
//                                 Err(Ok(tracker_error)) => {
//                                     println!(
//                                         "Host: {:?}, Tracker Error: {:?}",
//                                         &from_host, &tracker_error.message.0
//                                     );
//                                     continue;
//                                     // return Err((
//                                     //     3,
//                                     //     String::from_utf8(tracker_error.message.0)
//                                     //         .unify_error(4)?,
//                                     // ))
//                                 }
//                                 Err(Err(e)) => {
//                                     println!("Host: {:?}, Error: {:?}", &from_host, e);
//                                     continue; // Err((5, e)),
//                                 }
//                             };
//                             self.handle_announce_response(&from_host, announce_response)
//                                 .await;
//                         }
//                         TrackerState::AnnounceReceived => {}
//                         TrackerState::Established => {
//                             println!("Host: {:?}, Should not be established", &from_host)
//                         }
//                     }
//                 }
//                 Err(e) => {
//                     println!("Tracker Network Error: {:?}", e);
//                 }
//             }
//         }
//     }

//     fn update_state(&mut self, host: &SocketAddr, state: TrackerState) {
//         self.trackers.insert(host.clone(), state);
//     }

//     fn add_peers(&mut self, peers: impl IntoIterator<Item = SocketAddr>) {
//         let mut ps_write = self.peers.write().unwrap(); //.extend(peers);
//         for addr in peers {
//             if !ps_write.contains(&addr) {
//                 ps_write.push(addr);
//             }
//         }
//     }

//     async fn send_to(&self, target: &SocketAddr, buf: &Buffer) {
//         let sock = &self.sock;
//         let res = sock.send_to(buf, target).await;
//         match res {
//             Ok(sent_bytes) => {
//                 println!("tried: {}, sent: {}", buf.len(), sent_bytes);
//             }
//             Err(e) => {
//                 println!("Error: {:?}", e);
//             }
//         }
//     }

//     /// call after:  tracker added </br>
//     /// do:          send ConnectionRequest to host </br>
//     /// waiting:     ConnectionResponse </br>
//     async fn handle_tracker_init(&mut self, host: &SocketAddr) {
//         let transaction_id = rand::random();
//         let connect_request = ConnectRequest {
//             protocol_id: ConnectRequest::PROTOCOL_ID,
//             action: action::CONNECT,
//             transaction_id,
//         };

//         let mut buf = Box::new([0; BUFFER_SIZE]);

//         let byte_size = connect_request.byte_size();
//         connect_request.pack(buf.as_mut_slice());
//         self.send_to(host, &buf.as_slice()[..byte_size]).await;

//         self.update_state(host, TrackerState::ConnectSent(transaction_id));
//     }

//     /// call after:  </tab> received ConnectionResponse from host </br>
//     /// do:          send AnnounceRequest to host </br>
//     /// waiting:     AnnounceResponse </br>
//     async fn handle_connect_response(
//         &mut self,
//         host: &SocketAddr,
//         connect_response: ConnectResponse,
//     ) {
//         // TODO: handle transaction_id comparison on receive
//         // let Some(TrackerState::ConnectSent(transaction_id)) = self.trackers.get(host) else {
//         //     return; // TODO
//         // };

//         // let transaction_id = *transaction_id;

//         let transaction_id = rand::random();
//         let connection_id = connect_response.connection_id;

//         let announce_request = AnnounceRequest {
//             connection_id,
//             action: action::ANNOUNCE,
//             transaction_id,
//             info_hash: self.context.info_hash.clone(),
//             peer_id: self.context.self_peer_id.clone(),
//             // TODO: fill fields correctly
//             downloaded: 0,
//             left: 100,
//             uploaded: 0,
//             event: event::STARTED,
//             ip_address: 0,
//             key: 0,
//             num_want: -1,
//             port: 6881,
//         };

//         let mut buf = Box::new([0; BUFFER_SIZE]);

//         let byte_size = announce_request.byte_size();
//         announce_request.pack(buf.as_mut_slice());
//         self.send_to(host, &buf.as_slice()[..byte_size]).await;

//         self.update_state(host, TrackerState::AnnounceSent(transaction_id));
//     }

//     /// call after:  received AnnounceResponse from host </br>
//     /// do:          populate Peers </br>
//     /// waiting:     None </br>
//     async fn handle_announce_response(
//         &mut self,
//         host: &SocketAddr,
//         announce_response: Ipv4AnnounceResponse,
//     ) {
//         // TODO: handle transaction_id comparison on receive

//         let peers = announce_response.peers.iter().map(|peer| {
//             SocketAddr::V4(SocketAddrV4::new(
//                 Ipv4Addr::from(peer.ip_address),
//                 peer.port,
//             ))
//         });

//         println!("Adding peers: {:?}", &peers);
//         self.add_peers(peers);
//         println!(" -- Added -- ");
//         self.update_state(host, TrackerState::Established);
//     }
// }

// pub struct _TrackerClient {
//     context: Arc<TorrentContext>,
//     connection: UdpConnection,
//     url: Url,
//     client: Client,
//     // buffer: RefCell<[u8; BUFFER_SIZE]>,
// }

// impl _TrackerClient {
//     const USER_AGENT: &'static str = "win10:torrent-rs:0.1.0";

//     pub fn new(context: Arc<TorrentContext>, connection: UdpConnection, url: &str) -> Self {
//         let client = Client::builder()
//             .user_agent(Self::USER_AGENT)
//             .build()
//             .unwrap();
//         Self {
//             context,
//             connection,
//             url: Url::parse(url).unwrap(),
//             client: client,
//             // buffer: RefCell::new([0; BUFFER_SIZE]),
//         }
//     }

//     pub async fn send_http(
//         &self,
//         request: &TrackerRequest,
//     ) -> Result<TrackerResult, (usize, String)> {
//         self.client
//             .get(self.url.clone())
//             .query(&request.as_query())
//             .send()
//             .await
//             .unify_error(1)?
//             .bytes()
//             .await
//             .unify_error(2)?
//             .as_ref()
//             .apply(|b| {
//                 let s = String::from_utf8(b.to_vec());
//                 let _ = dbg!(s);
//                 b
//             })
//             .apply(serde_bencode::from_bytes)
//             .unify_error(3)
//     }

//     pub async fn send_udp(
//         &self,
//         // request: &TrackerRequest,
//     ) -> Result<Ipv4AnnounceResponse, (usize, String)> {
//         let mut buffer = [0; BUFFER_SIZE];
//         // let host = "tracker1.bt.moack.co.kr:80";
//         let host = self.url.host_str().unwrap().to_string()
//             + ":"
//             + self
//                 .url
//                 .port()
//                 .map(|p| p.to_string())
//                 .unwrap_or_else(|| "".to_string())
//                 .as_ref();
//         dbg!(&host);
//         // TODO: change to one to many
//         // let sock = UdpSocket::bind("0.0.0.0:6881").await.unify_error(1)?;
//         // sock.connect(&host).await.unify_error(2)?;

//         self.connection
//             .read()
//             .await
//             .get_buffer(&host.parse().unwrap());
//         todo!()

//         // println!("-- 1 --");

//         // let connect_response: ConnectResponse = {
//         //     let connect_request = ConnectRequest {
//         //         protocol_id: ConnectRequest::PROTOCOL_ID,
//         //         action: action::CONNECT,
//         //         transaction_id: rand::random(),
//         //     };

//         //     let byte_size = connect_request.byte_size();
//         //     connect_request.pack(buffer.as_mut_slice());
//         //     sock.send(&buffer.as_slice()[..byte_size]).await;

//         //     println!("-- 2 --");

//         //     let recv_n_bytes = sock.try_recv(buffer.as_mut_slice()).unify_error(100)?;

//         //     println!("-- 3 --");

//         //     let response_buf = &buffer[..recv_n_bytes];

//         //     let action_switch = ActionSwitch::unpack(&response_buf).unwrap();

//         //     let connect_response = action_switch.try_get_response(
//         //         &response_buf,
//         //         action::CONNECT,
//         //         connect_request.transaction_id,
//         //     );

//         //     match connect_response {
//         //         Ok(r) => r,
//         //         Err(Ok(tracker_error)) => {
//         //             return Err((
//         //                 3,
//         //                 String::from_utf8(tracker_error.message.0).unify_error(4)?,
//         //             ))
//         //         }
//         //         Err(Err(e)) => return Err((5, e)),
//         //     }
//         // };

//         // let connection_id = connect_response.connection_id;

//         // let announce_response: Ipv4AnnounceResponse = {
//         //     let announce_request = AnnounceRequest {
//         //         connection_id,
//         //         action: action::ANNOUNCE,
//         //         transaction_id: rand::random(),
//         //         info_hash: self.context.info_hash.clone(),
//         //         peer_id: self.context.peer_id.clone(),
//         //         downloaded: 0,
//         //         left: 100,
//         //         uploaded: 0,
//         //         event: event::STARTED,
//         //         ip_address: 0,
//         //         key: 0,
//         //         num_want: -1,
//         //         port: 6881,
//         //     };

//         //     let byte_size = announce_request.byte_size();
//         //     announce_request.pack(buffer.as_mut_slice());
//         //     sock.send(&buffer.as_slice()[..byte_size]).await;

//         //     println!("-- 4 --");

//         //     let recv_n_bytes = sock.try_recv(buffer.as_mut_slice()).unify_error(101)?;

//         //     println!("-- 5 --");

//         //     let response_buf = &buffer[..recv_n_bytes];

//         //     let action_switch = ActionSwitch::unpack(&response_buf).unwrap();

//         //     let announce_response = action_switch.try_get_response(
//         //         &response_buf,
//         //         action::ANNOUNCE,
//         //         announce_request.transaction_id,
//         //     );

//         //     match announce_response {
//         //         Ok(r) => r,
//         //         Err(Ok(tracker_error)) => {
//         //             return Err((
//         //                 3,
//         //                 String::from_utf8(tracker_error.message.0).unify_error(4)?,
//         //             ))
//         //         }
//         //         Err(Err(e)) => return Err((5, e)),
//         //     }
//         // };

//         // return Ok(announce_response);
//     }
// }

// fn unimpl_send<T>(request: &T) -> Vec<u8> {
//     unimplemented!()
// }
