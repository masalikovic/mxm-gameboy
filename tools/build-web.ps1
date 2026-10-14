# Build halaman web di Windows (PowerShell): gb-web → .wasm → lem JS, lalu rakit dist/.
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

# 3. Rakit dist/ dari web/ (sumber halaman), mulai dari folder kosong.
if (Test-Path dist) { Remove-Item dist -Recurse -Force }
Copy-Item web dist -Recurse

# 4. Buat lem JavaScript (ES module).
wasm-bindgen --target web --no-typescript --out-dir dist/pkg `
    target/wasm32-unknown-unknown/release/gb_web.wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 4b. ROM homebrew kita (kalau sudah dibuild) ikut disajikan sebagai demo
#     (ADR-0006). Tidak ada ROM pihak ketiga yang pernah disalin ke sini.
New-Item -ItemType Directory -Force dist/roms | Out-Null
Get-ChildItem homebrew/*/build/*.gb -ErrorAction SilentlyContinue | Copy-Item -Destination dist/roms

# 5. Cache busting (ADR-0009): ganti __BUILD__ dengan versi build.
$build = (git rev-parse --short=12 HEAD 2>$null)
if (-not $build) { $build = 'lokal' }
if (git status --porcelain 2>$null) { $build = "$build-$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())" }
foreach ($file in 'dist/index.html', 'dist/main.js', 'dist/input.js', 'dist/save.js', 'dist/audio.js') {
    $text = [IO.File]::ReadAllText((Resolve-Path $file)) -replace '__BUILD__', $build
    [IO.File]::WriteAllText((Resolve-Path $file), $text)
}

Write-Host "Selesai: dist/ siap (build $build). Coba lokal: cd dist; python -m http.server 8000"
