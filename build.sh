#!/usr/bin/env sh
set -eu

rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/implode_wasm.wasm game.wasm

rm -rf dist
mkdir -p dist
cp index.html style.css app.js game.wasm dist/
printf 'Built dist/ (%s)\n' "$(du -h dist/game.wasm | cut -f1) WebAssembly"
