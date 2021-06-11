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
relabuf = "~0.7.0"
```
