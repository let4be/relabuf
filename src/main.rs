use anyhow::Context;
use flume::{bounded, Sender};
use relabuf::{ExponentialBackoff, RelaBuf, RelaBufConfig};
use tokio::time::{sleep, Duration, Instant};

async fn producer(tx: Sender<u32>) {
    for i in 0..16 {
        let dur = Duration::from_millis(150_u64 * (i as u64));
        println!("waiting {:?} before emitting {}", &dur, i);
        sleep(dur).await;

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
