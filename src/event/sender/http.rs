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
    url: super::EnvString
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
    async fn send(&self, payload: Payload, state: &crate::event::process::State) -> Result<()> {
        let ps = self.config.http.iter()
            .map(|s| {
                match s {
                    HttpSenderType::Post { post } => {
                        // todo: handle missing url
                        let url = post.url.to_string(state).unwrap_or(String::from("missing url"));

                        log::debug!("sending HTTP POST to \"{}\" with body {:?}", url, payload.content);

                        // todo: handle error
                        let request = self.client
                            .post(&url)
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
                let resp = p.expect("http request failed");
                if !http::StatusCode::from(resp.status()).is_success() {
                    log::error!("http call to {} failed with code {}", resp.url(), resp.status())
                }
            });

        Ok(())
    }
}