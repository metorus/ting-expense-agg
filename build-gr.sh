#!/usr/bin/bash
echo === Building graphics,server ===
cargo build --features graphics,server
echo === Building graphics,selfhost ===
cargo build --features eframe/__screenshot,graphics,selfhost
