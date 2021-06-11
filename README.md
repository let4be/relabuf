![crates.io](https://img.shields.io/crates/v/relabuf.svg)
[![Dependency status](https://deps.rs/repo/github/let4be/relabuf/status.svg)](https://deps.rs/repo/github/let4be/relabuf)

# Relabuf - release buffer

Releases content after
 - a configurable time threshold is reached and buffer is not empty
 - a configurable number of items were added
 - exposes a notification via a future(must be selected on)

## Install

```
[dependencies]
relabuf = "~0.5.0"
```
