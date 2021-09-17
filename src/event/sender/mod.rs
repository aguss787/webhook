mod http;

use thiserror::Error;
use async_trait::async_trait;
use serde::Deserialize;

#[async_trait]
pub trait Sender {
    async fn send(&self, payload: Payload) -> Result<()>;
}

pub struct Payload {
    pub content: Vec<u8>
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum SenderConfig {
    Http(http::HttpSenderConfig)
}

#[derive(Error, Debug)]
pub enum Error {
}

type Result<T> = std::result::Result<T, Error>;

pub fn new_sender(config: &SenderConfig) -> Result<Box<dyn Sender>> {
    Ok(
        match config {
            SenderConfig::Http(c) => { Box::new(http::HttpSender::new(c)) }
        }
    )
}