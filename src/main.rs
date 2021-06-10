use flume::{bounded, Sender};
use relabuf::RelaBuf;
use std::time::Duration;
use tokio::time::sleep;

async fn produce(tx: Sender<u32>) {
    for i in 0..16 {
        sleep(Duration::from_millis(150_u64 * (i as u64))).await;
        let _ = tx.send_async(i).await;
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = bounded(100);

    tokio::spawn(produce(tx));

    let mut buf = RelaBuf::new(Duration::from_secs(5), 3);
    while !rx.is_disconnected() {
        tokio::select! {
            item = rx.recv_async() => {
                let _ = buf.add(item);
            }
            wake_reason = buf.wake() => {
                if let Some(consumed) = buf.try_consume() {
                    println!("consumed {:?} because {:?}, since last consumption {:?}", consumed.items, wake_reason, consumed.elapsed);
                }
            }
        }
    }

    if let Some(consumed) = buf.try_consume() {
        println!("finished with consumed {:?}", consumed);
    }
}
