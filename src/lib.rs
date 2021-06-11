use anyhow::anyhow;
use backoff::backoff::Backoff;
use flume::{bounded, Receiver};
use futures_lite::Future;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub type PinnedFut<'a, T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type Result<T> = anyhow::Result<T>;

#[derive(Debug, PartialEq)]
pub enum Reason {
    Time,
    Size,
    Term,
}

#[derive(Debug)]
struct Consumed<T> {
    elapsed: Duration,
    items: Vec<T>,
}

pub struct Released<T> {
    pub reason: Reason,
    pub elapsed: Duration,
    pub items: Vec<T>,
    state: Arc<Mutex<State<T>>>,
}

impl<T> Released<T> {
    pub fn return_on_err(&self, items: Vec<T>) {
        let mut state = self.state.lock().unwrap();
        state.return_on_err(items);
    }
}

pub struct RelaBufConfig {
    pub release_after: Duration,
    pub soft_cap: usize,
    pub hard_cap: usize,
    pub backoff: Option<backoff::ExponentialBackoff>,
}

struct State<T> {
    buffer: Vec<T>,
    opts: RelaBufConfig,

    last_ok_consume: Instant,
    err: Option<anyhow::Error>,

    next_backoff: Option<Duration>,
}

impl<T> State<T> {
    fn new(opts: RelaBufConfig) -> Self {
        Self {
            buffer: vec![],
            opts,
            last_ok_consume: Instant::now(),
            err: None,
            next_backoff: None,
        }
    }

    pub fn is_at_soft_cap(&self) -> bool {
        self.buffer.len() >= self.opts.soft_cap
    }

    pub fn add_item(&mut self, item: T) {
        self.buffer.push(item)
    }

    pub fn return_on_err(&mut self, items: Vec<T>) {
        self.buffer.extend(items);
        if let Some(backoff) = self.opts.backoff.as_mut() {
            self.next_backoff = backoff.next_backoff();
        }
    }

    fn set_err(&mut self, err: anyhow::Error) {
        self.err = Some(err)
    }

    fn is_ready(&self) -> Option<Reason> {
        if self.buffer.is_empty() {
            if self.err.is_some() {
                return Some(Reason::Term);
            }

            return None;
        }
        if let Some(next_backoff) = self.next_backoff {
            if self.last_ok_consume.elapsed() < next_backoff {
                return None;
            }
        }

        if self.err.is_some() {
            return Some(Reason::Term);
        }

        if self.buffer.len() >= self.opts.soft_cap {
            return Some(Reason::Size);
        }

        if self.last_ok_consume.elapsed() >= self.opts.release_after {
            return Some(Reason::Time);
        }

        None
    }

    fn consume(&mut self) -> Consumed<T> {
        if let Some(backoff) = self.opts.backoff.as_mut() {
            backoff.reset();
        }

        let elapsed = self.last_ok_consume.elapsed();
        self.last_ok_consume = Instant::now();
        Consumed {
            elapsed,
            items: self.buffer.drain(0..).collect(),
        }
    }
}

pub struct RelaBuf<T: 'static + Send + Sync + std::fmt::Debug> {
    rx_buffer: Receiver<T>,
    state: Arc<Mutex<State<T>>>,
}

impl<T: Send + Sync + std::fmt::Debug> RelaBuf<T> {
    pub fn new<'a, F: 'static + Send + Fn() -> PinnedFut<'a, Result<T>>>(
        opts: RelaBufConfig,
        recv: F,
    ) -> Self {
        let (tx_buffer, rx_buffer) = bounded::<T>(opts.hard_cap);

        let state = Arc::new(Mutex::new(State::new(opts)));

        {
            tokio::spawn(async move {
                while !tx_buffer.is_disconnected() {
                    tokio::select! {
                        item = recv() => {
                            if let Ok(item) = item {
                                if tx_buffer.send_async(item).await.is_err() {
                                    break
                                }
                            } else {
                                break
                            }
                        }
                    }
                }
            });
        }

        Self { rx_buffer, state }
    }

    pub fn next(&self) -> PinnedFut<'static, Result<Released<T>>> {
        let state = Arc::clone(&self.state);
        let rx_buffer = self.rx_buffer.clone();

        Box::pin(async move {
            let reason = loop {
                if let Some(reason) = state.lock().unwrap().is_ready() {
                    break reason;
                }

                let t = tokio::time::sleep(Duration::from_millis(100));
                let is_err = { state.lock().unwrap().err.is_some() };
                let is_soft_cap = { state.lock().unwrap().is_at_soft_cap() };
                if is_soft_cap || is_err {
                    tokio::select! {
                        _ = t => {}
                    }
                } else {
                    tokio::select! {
                        r = rx_buffer.recv_async() => {
                            match r {
                                Ok(item) => state.lock().unwrap().add_item(item),
                                Err(err) => state.lock().unwrap().set_err(anyhow!("cannot read from buffer channel: {}",err))
                            }
                        },

                        _ = t => {}
                    }
                }
            };

            let mut s = state.lock().unwrap();
            let consumed = s.consume();
            if reason == Reason::Term && consumed.items.is_empty() {
                return Err(s.err.take().unwrap());
            }
            Ok(Released {
                reason,
                elapsed: consumed.elapsed,
                items: consumed.items,
                state: Arc::clone(&state),
            })
        })
    }
}
