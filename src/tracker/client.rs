use reqwest::{Client, RequestBuilder};
use tokio::net::UdpSocket;
use url::Url;

use crate::{
    data::tracker::{TrackerRequest, TrackerResult},
    util::{ApplyTransform, UnifyError},
};

pub struct TrackerClient {
    url: Url,
    client: Client,
}

impl TrackerClient {
    const USER_AGENT: &'static str = "win10:torrent-rs:0.1.0";

    pub fn new(url: &str) -> Self {
        let client = Client::builder()
            .user_agent(Self::USER_AGENT)
            .build()
            .unwrap();
        Self {
            url: Url::parse(url).unwrap(),
            client: client,
        }
    }

    pub async fn send_http(
        &self,
        request: &TrackerRequest,
    ) -> Result<TrackerResult, (usize, String)> {
        self.client
            .get(self.url.clone())
            .query(&request.as_query())
            .send()
            .await
            .unify_error(1)?
            .bytes()
            .await
            .unify_error(2)?
            .as_ref()
            .apply(|b| {
                let s = String::from_utf8(b.to_vec());
                let _ = dbg!(s);
                b
            })
            .apply(serde_bencode::from_bytes)
            .unify_error(3)
    }

    pub async fn send_udp(
        &self,
        request: &TrackerRequest,
    ) -> Result<TrackerResult, (usize, String)> {
        let sock = UdpSocket::bind("0.0.0.0:6881").await.unify_error(1)?;
        sock.connect(
            self.url.host_str().unwrap().to_string()
                + self
                    .url
                    .port()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "".to_string())
                    .as_ref(),
        )
        .await
        .unify_error(2)?;

        todo!()
    }
}
