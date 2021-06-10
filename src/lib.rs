use flume::{unbounded, Receiver, Sender};
use futures_lite::Future;
use std::{
    pin::Pin,
    time::{Duration, Instant},
};

pub type PinnedFut<'a, T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug)]
pub enum Reason {
    Time,
    Limit,
}

#[derive(Debug)]
pub struct Consumed<T> {
    pub elapsed: Duration,
    pub items: Vec<T>
}

pub struct RelaBuf<T: 'static + Send + Sync> {
    tx_wake_ch: Sender<Reason>,
    rx_wake_ch: Receiver<Reason>,
    tx_reset_time: Sender<()>,
    buffer: Vec<T>,
    buffer_max: usize,
    last_consumed: Instant
}

impl<T: Send + Sync> RelaBuf<T> {
    pub fn new(release_after: Duration, buffer_max: usize) -> Self {
        let (tx_wake_ch, rx_wake_ch) = unbounded::<Reason>();
        let (tx_reset_time, rx_reset_time) = unbounded::<()>();
        let s = Self { rx_wake_ch, buffer: vec![], buffer_max, tx_wake_ch: tx_wake_ch.clone(), tx_reset_time, last_consumed: Instant::now() };

        tokio::spawn(async move {
            while !tx_wake_ch.is_disconnected() {
                tokio::select! {
                    _ = tokio::time::sleep(release_after) => {
                        let _ = tx_wake_ch.send_async(Reason::Time).await;
                    }
                    _ = rx_reset_time.recv_async() => {}
                }
            }
        });

        s
    }

    pub fn add(&mut self, item: T) {
        self.buffer.push(item);
        if self.buffer.len() >= self.buffer_max {
            let _ = self.tx_wake_ch.send(Reason::Limit);
        }
    }

    pub fn try_consume(&mut self) -> Option<Consumed<T>> {
        if self.buffer.is_empty() {
            return None;
        }

        let _ = self.tx_reset_time.send(());

        let elapsed = self.last_consumed.elapsed();
        self.last_consumed = Instant::now();
        Some(Consumed{
            items: self.buffer.drain(0..).collect(),
            elapsed
        })
    }

    pub fn wake(&self) -> PinnedFut<'static, Reason> {
        let rx = self.rx_wake_ch.clone();
        Box::pin(async move {
            rx.recv_async().await.unwrap()
        })
    }
}
