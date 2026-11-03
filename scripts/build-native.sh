#!/bin/sh
# Builds the native tab to WebAssembly and puts it where the manifest's
# `entry` says: tabs/checklist.wasm. Hangar runs this file and never builds
# one, so it is committed, in the commit that changes the source it was built
# from; CI rebuilds it and fails when the committed one is not that build.
#
#   scripts/build-native.sh          build it
#   scripts/build-native.sh --check  build it, and fail if the committed one is not that
#
# Needs rustup (native/rust-toolchain.toml names the compiler and the
# wasm32-unknown-unknown target). Run from anywhere.
set -eu
cd "$(dirname "$0")/../native"

# The same bytes wherever it is built: no path of this machine in the module.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
export RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$(pwd)=/kit"

cargo build --release --locked --target wasm32-unknown-unknown
built=target/wasm32-unknown-unknown/release/kit_checklist_tab.wasm
if [ "${1:-}" = "--check" ]; then
  if cmp -s "$built" ../tabs/checklist.wasm; then
    echo "tabs/checklist.wasm is what native/ builds"
    exit 0
  fi
  echo "tabs/checklist.wasm is stale: run scripts/build-native.sh and commit the result" >&2
  exit 1
fi
cp "$built" ../tabs/checklist.wasm
echo "tabs/checklist.wasm: $(wc -c < ../tabs/checklist.wasm) bytes, sha256 $(sha256sum ../tabs/checklist.wasm | cut -d' ' -f1)"
