#!/usr/bin/env bash
# Build halaman web: gb-web (Rust) → .wasm → lem JavaScript di web/pkg/.
# Dipakai di CI dan di Linux/macOS. Versi Windows: tools/build-web.ps1.
# Langkah-langkahnya sengaja terlihat satu per satu (ADR-0003).
set -euo pipefail

cd "$(dirname "$0")/.."

# 1. Versi wasm-bindgen-cli harus sama persis dengan crate wasm-bindgen di
#    Cargo.lock. Kalau beda, file lem JS tidak cocok dengan .wasm dan halaman
#    gagal dimuat dengan error yang membingungkan. Lebih baik berhenti di sini.
expected="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen-cli belum terpasang. Pasang dengan:" >&2
  echo "  cargo install wasm-bindgen-cli --version ${expected} --locked" >&2
  exit 1
fi
installed="$(wasm-bindgen --version | awk '{ print $2 }')"
if [[ "${installed}" != "${expected}" ]]; then
  echo "Versi wasm-bindgen-cli ${installed} tidak sama dengan Cargo.lock (${expected})." >&2
  echo "  cargo install wasm-bindgen-cli --version ${expected} --locked --force" >&2
  exit 1
fi

# 2. Compile adapter browser ke WebAssembly.
cargo build -p gb-web --release --target wasm32-unknown-unknown

# 3. Buat lem JavaScript (ES module) yang memanggil fungsi #[wasm_bindgen].
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/gb_web.wasm

echo "Selesai: web/pkg/ siap. Coba lokal: cd web && python3 -m http.server 8000"
