#!/usr/bin/bash
echo === WASM bundle for Web and WebViews ===
cargo build --release --target wasm32-unknown-unknown --features graphics_wasm
echo === Server ===
cargo build --release --features server

