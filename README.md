![crates.io](https://img.shields.io/crates/v/relabuf.svg)
[![Dependency status](https://deps.rs/repo/github/let4be/relabuf/status.svg)](https://deps.rs/repo/github/let4be/relabuf)

# Relabuf - smart buffer with release valve
 - consumes items from external `future`
 - buffers internally up to `hard_cap`
 - when `hard_cap` is reached no longer consumes causing producer to backoff and slowdown
 - capable of releasing contents ONLY under certain conditions
 - a `release_after` has passed since the latest successful content release(or since start) and buffer is not empty
 - a `soft_cap` of items were added
 - each consumption should be either `confirmed` or `returned` to the buffer
 - returns usually happen due to error(for example DB is down) - so it's possible to configure a backoff
 - backoff essentially overrides time release valve
 - exposes released items via a `future` user can `await` on
## Install

```
[dependencies]
relabuf = "~0.14.0"
```

example:

```rust
use anyhow::Context;
use flume::{bounded, Sender};
use relabuf::{ExponentialBackoff, RelaBuf, RelaBufConfig};
use std::time::{Duration, Instant};
use async_io::Timer;

async fn producer(tx: Sender<u32>) {
    for i in 0..16 {
        let dur = Duration::from_millis(150_u64 * (i as u64));
        println!("waiting {:?} before emitting {}", &dur, i);
        Timer::interval(dur).await;

        let t = Instant::now();
        let r = tx.send_async(i).await;
        println!("emit for {} took {:?}: {:?}", i, t.elapsed(), r);
    }
    println!("producer is finished!")
}

#[tokio::main]
async fn main() {
    let (tx, rx) = bounded(5);

    tokio::spawn(producer(tx));

    let opts = RelaBufConfig {
        soft_cap: 3,
        hard_cap: 5,
        release_after: Duration::from_secs(5),
        backoff: Some(ExponentialBackoff {
            max_elapsed_time: None,
            ..ExponentialBackoff::default()
        }),
    };

    let buf = RelaBuf::new(opts, move || {
        let rx = rx.clone();
        Box::pin(async move { rx.recv_async().await.context("cannot read") })
    });
    
    tokio::spawn(buf.go());

    let mut i = 0;

    while let Ok(consumed) = buf.next().await {
        i += 1;

        if i <= 7 {
            println!(
                "consumed {:?} because {:?}, since last consumption {:?} - returning due to err",
                consumed.items, consumed.reason, consumed.elapsed
            );
            consumed.return_on_err();
        } else {
            println!(
                "consumed {:?} because {:?}, since last consumption {:?}",
                consumed.items, consumed.reason, consumed.elapsed
            );
            consumed.confirm();
        }
    }
    println!("done ;)");
}
```