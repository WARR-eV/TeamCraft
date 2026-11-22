#!/usr/bin/env bash
# Builds the TeamCraft plugin for SolidCraft: the package folder
# target/package/teamcraft/ with plugin.json and plugin.wasm (a WebAssembly component).
# Needs the wasm32-unknown-unknown target (rustup target add
# wasm32-unknown-unknown) and wasm-tools (cargo install wasm-tools).
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release --target wasm32-unknown-unknown
core=target/wasm32-unknown-unknown/release/teamcraft_solidcraft_plugin.wasm
out=target/package/teamcraft
rm -rf "$out"
mkdir -p "$out"
# the core module + the component type wit-bindgen embedded → a component
wasm-tools component new "$core" -o "$out/plugin.wasm"
wasm-tools validate "$out/plugin.wasm"
# SolidCraft refuses WASI and anything outside craft:plugin@0.2.0
imports=$(wasm-tools component wit "$out/plugin.wasm" | grep -E '^\s*import ' || true)
echo "Imports:"
echo "$imports"
if grep -vqE 'craft:plugin/[a-z-]+@0\.2\.0' <<<"$imports"; then
    echo "error: the component imports something other than craft:plugin@0.2.0" >&2
    exit 1
fi
cp plugin.json README.md "$out/"
cp ../../LICENSE "$out/LICENSE"
(cd "$out" && sha256sum plugin.json plugin.wasm > SHA256SUMS)
echo "Package: $out"
