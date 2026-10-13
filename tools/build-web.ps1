# Build halaman web di Windows (PowerShell): gb-web → .wasm → lem JS di web/pkg/.
# Isinya sama dengan tools/build-web.sh; lihat komentar di sana untuk alasannya.
# Jalankan dari folder repo:  .\tools\build-web.ps1
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# 1. Versi wasm-bindgen-cli harus sama persis dengan Cargo.lock (ADR-0003).
$lock = Get-Content Cargo.lock
$index = [Array]::IndexOf($lock, 'name = "wasm-bindgen"')
$expected = ($lock[$index + 1] -replace 'version = "(.*)"', '$1')
if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
    Write-Error "wasm-bindgen-cli belum terpasang. Pasang dengan: cargo install wasm-bindgen-cli --version $expected --locked"
}
$installed = ((wasm-bindgen --version) -split ' ')[1]
if ($installed -ne $expected) {
    Write-Error "Versi wasm-bindgen-cli $installed tidak sama dengan Cargo.lock ($expected). Pasang ulang: cargo install wasm-bindgen-cli --version $expected --locked --force"
}

# 2. Compile adapter browser ke WebAssembly.
cargo build -p gb-web --release --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 3. Buat lem JavaScript (ES module).
wasm-bindgen --target web --no-typescript --out-dir web/pkg `
    target/wasm32-unknown-unknown/release/gb_web.wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host 'Selesai: web/pkg/ siap. Coba lokal: cd web; python -m http.server 8000'
