use async_trait::async_trait;
use crate::event::sender::{Sender, Payload, Result};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct HttpSenderConfig {
    http: Vec<HttpSenderType>
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum HttpSenderType {
    Post { post: HttpSenderUrlConfig },
}

#[derive(Deserialize, Clone, Debug)]
struct HttpSenderUrlConfig {
    url: String
}

pub struct HttpSender {
    config: HttpSenderConfig,
    client: reqwest::Client,
}

impl HttpSender {
    pub fn new(config: &HttpSenderConfig) -> Self {
        HttpSender{
            config: config.clone(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Sender for HttpSender {
    async fn send(&self, payload: Payload) -> Result<()> {
        let ps = self.config.http.iter()
            .map(|s| {
                match s {
                    HttpSenderType::Post { post } => {
                        log::debug!("sending HTTP POST to {} with body {:?}", post.url, payload.content);

                        // todo: handle error
                        let request = self.client
                            .post(&post.url)
                            .body(payload.content.clone())
                            .build()
                            .expect("unable to build request");

                        self.client.execute(request)
                    } }
            });

        futures::future::join_all(ps).await
            .drain(0..)
            .for_each(|p| {
                // todo: handle error
                p.expect("http request failed");
            });

        Ok(())
    }
}