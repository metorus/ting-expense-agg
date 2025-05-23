#!/usr/bin/bash
echo === Checking graphics,server ===
cargo check --features graphics_nowasm,server
echo === Checking graphics,selfhost ===
cargo check --features eframe/__screenshot,graphics_nowasm,selfhost
