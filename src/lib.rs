use flume::{unbounded, Receiver, Sender};
use futures_lite::Future;
use std::{pin::Pin, time::Duration};

pub type PinnedFut<'a, T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug)]
pub enum Reason {
    Time,
    Limit,
}

pub struct RelaBuf<T: 'static + Send + Sync> {
    tx_wake_ch: Sender<Reason>,
    rx_wake_ch: Receiver<Reason>,
    buffer: Vec<T>,
    buffer_max: usize
}

impl<T: Send + Sync> RelaBuf<T> {
    pub fn new(release_after: Duration, buffer_max: usize) -> Self {
        let (tx_wake_ch, rx_wake_ch) = unbounded::<Reason>();
        let s = Self { rx_wake_ch, buffer: vec![], buffer_max, tx_wake_ch: tx_wake_ch.clone() };

        tokio::spawn(async move {
            while !tx_wake_ch.is_disconnected() {
                tokio::time::sleep(release_after).await;
                let _ = tx_wake_ch.send_async(Reason::Time).await;
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

    pub fn try_consume(&mut self) -> Option<Vec<T>> {
        if self.buffer.is_empty() {
            return None;
        }

        Some(self.buffer.drain(0..).collect())
    }

    pub fn wake(&self) -> PinnedFut<'static, Reason> {
        let rx = self.rx_wake_ch.clone();
        Box::pin(async move {
            rx.recv_async().await.unwrap()
        })
    }
}
