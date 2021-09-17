pub trait GracefulSignalInvoker: Send {
    fn call(&self);
}

pub fn new_graceful_signal() -> (SingleGracefulSignalInvoker, GracefulSignal) {
    let (s, r) = crossbeam_channel::unbounded();
    (SingleGracefulSignalInvoker{s}, GracefulSignal{r})
}

pub struct GracefulSignal {
    r: crossbeam_channel::Receiver<()>,
}

impl GracefulSignal {
    pub async fn called(&self) {
        let r = self.r.clone();
        tokio::spawn(async move {
            if let Err(e) = r.recv() {
                log::warn!("graceful signal is received with an channel error: {}", e);
            }
        }).await;
    }
}

pub struct SingleGracefulSignalInvoker {
    s: crossbeam_channel::Sender<()>,
}

impl GracefulSignalInvoker for SingleGracefulSignalInvoker {
    fn call(&self) {
        if let Err(e) = self.s.send(()) {
            log::error!("graceful signal is sent with an error: {}", e);
        };
    }
}

pub fn combine(v: Vec<Box<dyn GracefulSignalInvoker>>) -> CombinedGracefulSignalInvoker {
    CombinedGracefulSignalInvoker {
        v
    }
}

pub struct CombinedGracefulSignalInvoker {
    v: Vec<Box<dyn GracefulSignalInvoker>>
}

impl GracefulSignalInvoker for CombinedGracefulSignalInvoker {
    fn call(&self) {
        self.v.iter().for_each(|g| g.call());
    }
}