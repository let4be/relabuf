use anyhow::Context;
use flume::{bounded, Sender};
use relabuf::{RelaBuf, RelaBufConfig};
use std::time::Duration;
use tokio::time::sleep;

async fn producer(tx: Sender<u32>) {
    for i in 0..16 {
        sleep(Duration::from_millis(150_u64 * (i as u64))).await;
        let _ = tx.send_async(i).await;
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = bounded(100);

    tokio::spawn(producer(tx));

    let opts = RelaBufConfig {
        soft_cap: 3,
        hard_cap: 5,
        release_after: Duration::from_secs(5),
        backoff: Some(backoff::ExponentialBackoff::default()),
    };

    let buf = RelaBuf::new(opts, move || {
        let rx = rx.clone();
        Box::pin(async move { rx.recv_async().await.context("cannot read") })
    });
    while let Ok(consumed) = buf.next().await {
        println!(
            "consumed {:?} because {:?}, since last consumption {:?}",
            consumed.items, consumed.reason, consumed.elapsed
        );
    }
    println!("done ;)");
}
