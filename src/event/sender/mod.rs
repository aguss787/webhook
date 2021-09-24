mod http;

use thiserror::Error;
use async_trait::async_trait;
use serde::Deserialize;
use crate::event::process::Identifier;

#[async_trait]
pub trait Sender {
    async fn send(&self, payload: Payload, state: &crate::event::process::State) -> Result<()>;
}

#[derive(Clone)]
pub struct Payload {
    pub content: Vec<u8>
}

impl Payload {
    pub fn new(content: Vec<u8>) -> Self {
        Payload{ content }
    }
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

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum EnvString {
    FromEnv { from_env: Identifier },
    String(String),
}

impl EnvString {
    fn to_string(&self, state: &crate::event::process::State) -> Option<String> {
        match self {
            EnvString::FromEnv { from_env: key } => {
                log::debug!("getting string from env with key: {}", key);
                let val = state.get(key);
                match val {
                    Some(crate::event::process::Item::Value(crate::event::process::Value::StringValue(s))) => {
                        log::debug!("string from env with key \"{}\" found: {}", key, s);
                        Some(s.clone())
                    },
                    _ => None,
                }
            },
            EnvString::String(s) => { Some(s.clone()) },
        }
    }
}
