use serde::Deserialize;
use thiserror::Error;

use process::operation;
pub use utils::sync::GracefulSignalInvoker;

use crate::event::trigger::SourceEvent;
use crate::event::utils::sync::{combine, GracefulSignal, new_graceful_signal};

mod trigger;
mod utils;
mod queue;
mod sender;
mod process;

#[derive(Deserialize, Debug, Clone)]
pub struct Event {
    name: String,
    trigger: Vec<trigger::Trigger>,
    process: Option<Vec<operation::Op>>,
    target: Vec<sender::SenderConfig>,
}

pub fn load_events(dir: &String) -> Vec<Event> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter(|f| {
            match f {
                Ok(_) => {}
                Err(e) => { log::warn!("unable to read file/directory: {}", e) }
            };

            f.is_ok()
        })
        .map(|f| f.unwrap())
        .filter(|f| f.path().is_file())
        .map(|f| f.path().to_str().unwrap().to_string())
        .map(|f| {
            log::trace!("reading {}", f);
            // todo: handle error
            std::fs::read_to_string(f).expect("unable to read file")
        })
        // todo: handle yaml error
        .map(|f| serde_yaml::from_str(f.as_str()).expect("unable to parse config"))
        .collect()
}

pub struct Executor {}

impl Executor {
    pub fn new() -> Self {
        Executor {}
    }

    pub fn start(&self, mut events: Vec<Event>) -> (impl std::future::Future, Box<dyn GracefulSignalInvoker>) {
        let (promises, invokers): (Vec<_>, Vec<_>) = events
            .drain(0..)
            .map(|e| Pipeline::new(e))
            .map(|p| p.start())
            .unzip();

        (
            futures::future::join_all(promises),
            Box::new(combine(invokers)),
        )
    }
}

pub struct Pipeline {
    event: Event,
}

impl Pipeline {
    pub fn new(event: Event) -> Self {
        Pipeline {
            event,
        }
    }

    pub fn start(&self) -> (impl std::future::Future, Box<dyn GracefulSignalInvoker>) {
        log::info!("starting pipeline for {}", self.event.name);
        let (i, s) = new_graceful_signal();

        (Self::start_loop(self.event.clone(), s), Box::new(i))
    }

    async fn start_loop(event: Event, graceful_signal: GracefulSignal) {
        let graceful_stop = graceful_signal.called();
        tokio::pin!(graceful_stop);

        let (queue_sender, queue_receiver) = queue::new_queue(Some(0));

        let triggers = event.trigger.iter()
            .map(|t| trigger::new_source_event_receiver(t).expect("unable to initialize event receiver"))
            .map(|r| (r, queue_sender.clone()))
            .map(|(r, s)| {
                tokio::spawn(async move {
                    loop {
                        let event = r.get_one().await.expect("unable to retrieve event");
                        let s = s.clone();
                        let res = tokio::task::spawn(async move {
                            s.send(event)
                        }).await;

                        if let Err(e) = res {
                            log::error!("event sender thread join error: {}", e);
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        let senders = event.target.iter()
            // todo: handle error
            .map(|t| sender::new_sender(t).expect("unable to create sender"))
            .collect::<Vec<_>>();

        let ops = match &event.process {
            None => { vec!() }
            Some(ops) => { ops.clone() }
        };

        loop {
            let queue_receiver = queue_receiver.clone();
            let new_message = tokio::task::spawn(async move {
                queue_receiver.recv()
            });

            log::trace!("pipeline {} waiting for new message or stop signal", event.name);
            tokio::select! {
                _ = &mut graceful_stop => { log::debug!("pipeline {} receive stop signal", event.name); break},
                msg = new_message => {
                    let msg = msg.unwrap();
                    log::debug!("new message {:?}", String::from_utf8(msg.bytes().clone()));

                    let res = dispatch_webhook(&event, &senders, &msg, &ops).await;
                    if let Err(e) = res {
                        log::error!("error dispatching webhook: {}", e)
                    }
                    msg.done().await;
                },
            }
            ;
            log::trace!("pipeline {} done waiting for new message or stop signal", event.name);
        }

        for trigger in triggers {
            let res = trigger.await;
            if let Err(e) = res {
                log::error!("error joining trigger thread: {}", e);
            }
        }
        log::info!("pipeline {} stopped", event.name);
    }
}

#[derive(Error, Debug)]
enum Error {}

type Result<T> = std::result::Result<T, Error>;

async fn dispatch_webhook(
    event: &Event, senders: &Vec<Box<dyn sender::Sender>>,
    msg: &Box<dyn SourceEvent>,
    ops: &Vec<operation::Op>,
) -> Result<()> {
    let (payload, state) = ops.iter()
        .fold((sender::Payload { content: msg.bytes().clone() }, process::State::new()), |(payload, state), op| {
            let (payload, new_state) = op.execute(payload, state).expect("unhandled error on process execution");
            log::trace!("pipeline \"{}\" new state: {:?}", event.name, new_state);
            (payload, new_state)
        });

    let ps = senders.iter()
        .map(|s| {
            s.send(payload.clone(), &state)
        });

    let ps = futures::future::join_all(ps).await;
    // todo: handle error
    ps.iter().for_each(|p| {
        p.as_ref().expect("failed to send message");
    });
    Ok(())
}