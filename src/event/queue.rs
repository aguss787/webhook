pub fn new_queue<T>(buffer: Option<usize>) -> (QueuePusher<T>, QueuePuller<T>) {
    let (s, r) = match buffer {
        None => crossbeam_channel::unbounded(),
        Some(x) => crossbeam_channel::bounded(x),
    };

    (QueuePusher{s}, QueuePuller{r})
}

#[derive(Debug)]
pub struct QueuePusher<T> {
    s: crossbeam_channel::Sender<T>
}

impl<T> QueuePusher<T> {
    pub fn send(&self, o: T) {
        log::trace!("sending an entry to the queue");

        // todo: error handling
        self.s.send(o).expect("unable to send message");
    }
}

impl<T> Clone for QueuePusher<T> {
    fn clone(&self) -> Self {
        QueuePusher{
            s: self.s.clone(),
        }
    }
}

#[derive(Debug)]
pub struct QueuePuller<T> {
    r: crossbeam_channel::Receiver<T>
}

impl<T> Clone for QueuePuller<T> {
    fn clone(&self) -> Self {
        QueuePuller{
            r: self.r.clone(),
        }
    }
}

impl<T> QueuePuller<T> {
    pub async fn recv(&self) -> T {
        log::trace!("receiving an entry in the queue");
        // todo: error handling
        self.r.recv().expect("unable to get message")
    }
}