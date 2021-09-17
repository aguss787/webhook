mod pubsub;

use serde::{Deserialize};
use thiserror::Error;

#[derive(Deserialize, Debug, Clone)]
pub struct Trigger {
    #[serde(rename = "type")]
    trigger_type: String,

    config: Option<serde_yaml::Value>
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("unknown trigger type: {0}")]
    UnknownType(String),

    #[error("invalid credential: {0}")]
    InvalidCredential(String),

    #[error("failed to pull data: {0}")]
    PullError(String)
}

type Result<T> = std::result::Result<T, Error>;

use async_trait::async_trait;

#[async_trait]
pub trait SourceEventReceiver: Send + Sync {
    async fn get_one(&self) -> Result<Box<dyn SourceEvent>>;
}

#[async_trait]
pub trait SourceEvent: Send + Sync {
    fn bytes(&self) -> &Vec<u8>;
    async fn done(&self);
}

pub fn new_source_event_receiver(trigger: &Trigger) -> Result<Box<dyn SourceEventReceiver>> {
    match trigger.trigger_type.as_str() {
        "google-pubsub" => Ok(Box::new(pubsub::Receiver::new(trigger)?)),
        t => Err(Error::UnknownType(t.to_string())),
    }
}
