use crate::event::trigger::{Trigger, SourceEvent, SourceEventReceiver};
use serde::Deserialize;
use super::{Result, Error};
use google_pubsub1::Pubsub;
use google_pubsub1::api::{PullRequest, AcknowledgeRequest, ReceivedMessage};

pub struct Receiver {
    pubsub: Pubsub,
    subscription_id: String,
}

#[derive(Deserialize)]
struct PubSubConfig {
    credential: String,
    subscription_id: String,
}

impl Receiver {
    pub fn new(trigger: &Trigger) -> Result<Self> {
        let config: PubSubConfig = trigger.config.clone()
            .map(|v| serde_yaml::from_value(v))
            .ok_or(Error::InvalidConfig("missing config".to_string()))?
            .map_err(|e| Error::InvalidConfig(format!("{}", e)))?;

        log::debug!("initializing pubsub receiver for subscription \"{}\"", config.subscription_id);

        let secret: yup_oauth2::ServiceAccountKey = serde_json::from_str(config.credential.as_str())
            .map_err(|e| Error::InvalidCredential(format!("{}", e)))?;

        let auth = futures::executor::block_on(async {
            yup_oauth2::ServiceAccountAuthenticator::builder(
                secret,
            ).build().await
        }).expect("failed to create pubsub authenticator");

        let hub = Pubsub::new(hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()), auth);

        log::debug!("pubsub receiver for subscription \"{}\" initialized", config.subscription_id);

        Ok(Receiver{
            pubsub: hub,
            subscription_id: config.subscription_id,
        })
    }
}

use async_trait::async_trait;

#[async_trait]
impl SourceEventReceiver for Receiver {
    async fn get_one(&self) -> Result<Box<dyn SourceEvent>> {
        let mut wait_time: f64 = 1.0;

        let message: ReceivedMessage = loop {
            let (_, resp) = {
                log::trace!("pulling message from pubsub ({})", self.subscription_id);
                self.pubsub
                    .projects()
                    .subscriptions_pull(
                        PullRequest{ max_messages: Some(1), return_immediately: Some(true) },
                        self.subscription_id.as_str(),
                    )
                    .doit()
                    .await
            }
                .map_err(|e| Error::PullError(format!("{}", e)))?;

            log::trace!("pubsub ({}) responses: {:?}", self.subscription_id, resp);
            match resp.received_messages {
                None => {
                    tokio::time::sleep(tokio::time::Duration::new(wait_time.floor() as u64, 0)).await;
                    wait_time = wait_time * 1.25;
                    if wait_time > 10.0 {
                        wait_time = 10.0;
                    }
                },
                Some(mut messages) => {
                    let content = messages.pop();
                    if content.is_some() {
                        let c = content.unwrap();
                        break c;
                    }
                },
            }
        };

        let content = message.message.expect("unable to get pubsub message").data.expect("empty pubsub data");
        let content = base64::decode(content).expect("unable to decode pubsub message");
        log::trace!("pubsub ({}) received: {:?}", self.subscription_id, content);

        Ok(
            Box::new(
                Event{
                    content,
                    pubsub: self.pubsub.clone(),
                    ack_id: message.ack_id.expect("missing ack_id"),
                    subscription_id: self.subscription_id.clone(),
                }
            )
        )
    }
}

struct Event {
    content: Vec<u8>,

    pubsub: Pubsub,
    ack_id: String,
    subscription_id: String,
}

#[async_trait]
impl SourceEvent for Event {
    fn bytes(&self) -> &Vec<u8> {
        &self.content
    }

    async fn done(&self) {
        log::trace!("ack-ing pubsub message with ack-id {}", self.ack_id);
        let ack_result = self.pubsub.projects()
            .subscriptions_acknowledge(
                AcknowledgeRequest{ ack_ids: Some(vec!(self.ack_id.clone())) },
                self.subscription_id.as_str(),
            )
            .doit()
            .await;

        // todo: propagate forward
        if let Err(e) = ack_result {
            log::error!("error ack-ing pubsub message with ack-id {}: {}", self.ack_id, e);
        } else {
            log::trace!("message with ack-id {} ack-ed", self.ack_id);
        }
    }
}