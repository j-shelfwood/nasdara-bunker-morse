# Contributing

Bug reports with a decoder log help most, especially from other audio setups and servers. Start the app with
`--log decode.log --trace` and attach both `decode.log` and `decode.log.trace.jsonl`. The logs hold beep timings and
digits, no audio.

Pull requests are welcome. Before opening one, run:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The decoder crate (`crates/decoder`) builds and tests on any platform. The app crate needs Windows for live capture.
