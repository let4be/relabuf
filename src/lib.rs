use anyhow::Context;
use flume::{unbounded, Receiver, Sender};
use futures_lite::Future;
use std::{pin::Pin, time::Duration};

type Result<T> = anyhow::Result<T>;
pub type PinnedFut<'a, T = ()> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub struct RelaBuf<T: 'static + Send + Sync> {
    tx_add: Sender<T>,
    rx_wake_ch: Receiver<Vec<T>>,
}

impl<T: Send + Sync> RelaBuf<T> {
    pub fn new(release_after: Duration, buffer_max: usize) -> Self {
        let (tx_wake_ch, rx_wake_ch) = unbounded::<Vec<T>>();
        let (tx_add, rx_add) = unbounded::<T>();
        tokio::spawn(async move {
            let mut buffer = vec![];
            let mut sleep = Box::pin(tokio::time::sleep(release_after));
            loop {
                tokio::select! {
                    Ok(item) = rx_add.recv_async() => {
                        if buffer.len() >= buffer_max && tx_wake_ch.send(buffer.drain(0..).collect()).is_err(){
                            break
                        }
                        buffer.push(item);
                    },
                    _ = &mut sleep => {
                        if !buffer.is_empty() && tx_wake_ch.send(buffer.drain(0..).collect()).is_err(){
                            break
                        }
                        sleep = Box::pin(tokio::time::sleep(release_after))
                    }
                }
            }
        });

        Self { tx_add, rx_wake_ch }
    }

    pub fn add(&mut self, item: T) -> Result<()> {
        self.tx_add
            .send(item)
            .context("cannot send to background channel")
    }

    pub fn wake(&self) -> PinnedFut<'static, Vec<T>> {
        let rx = self.rx_wake_ch.clone();
        Box::pin(async move { Ok(rx.recv_async().await?) })
    }
}
