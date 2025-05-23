#!/usr/bin/bash
echo === WASM bundle for Web and WebViews ===
cargo build --release --target wasm32-unknown-unknown --features graphics_wasm
~/.cargo/bin/wasm-bindgen --no-typescript --target web --out-dir assets ./target/wasm32-unknown-unknown/release/ting-expense-a.wasm
echo === Server ===
cargo build --release --features server

