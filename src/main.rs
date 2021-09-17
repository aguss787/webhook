mod event;

use serde::Deserialize;
use crate::event::GracefulSignalInvoker;

#[derive(Deserialize, Debug)]
struct Config {
    webhook_events_dir: Option<String>,
    webhook_log_level: Option<String>,
}

#[tokio::main]
async fn main() {
    let config: Config = envy::from_env().expect("unable to load env");

    println!("{:?}", config);

    let logger = env_logger::Builder::new()
        .filter_level(log::LevelFilter::Trace)
        .build();

    log::set_boxed_logger(Box::new(logger)).expect("unable to set logger");

    let log_level = config.webhook_log_level.clone().unwrap_or("warn".to_string());

    log::set_max_level(
        match log_level.as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            "off" => log::LevelFilter::Off,
            _ => log::LevelFilter::Warn,
        }
    );

    log::debug!("config: {:?}", config);

    let events_dir = config.webhook_events_dir.unwrap_or("events".to_string());
    let events = event::load_events(&events_dir);

    log::debug!("events: {:?}", events);

    let executor = event::Executor::new();
    let (p, g) = executor.start(events);

    handle_signal(g);

    p.await;

    log::info!("webhook turned off");
}

#[cfg(all(not(windows)))]
fn handle_signal(g: Box<dyn GracefulSignalInvoker>) {
    let mut signals = signal_hook::iterator::Signals::new(&[
        signal_hook::consts::SIGTERM,
    ]).expect("unable to initialize signal handler");

    tokio::spawn(async move {
        for _ in signals.forever() {
            g.call();
            break;
        }
    });
}

#[cfg(windows)]
fn handle_signal(g: Box<dyn GracefulSignalInvoker>) {
    log::warn!("signal is not yet handled in windows");

    let (s, r) = crossbeam_channel::unbounded();
    tokio::spawn(async move {
        r.recv().unwrap();
        s.send(()).unwrap();
        g.call();
    });
}