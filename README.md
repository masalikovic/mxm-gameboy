# mxm-gameboy

Emulator Game Boy (DMG) dalam **Rust → WebAssembly**, bisa dimainkan di browser HP,
plus program homebrew dalam **C (GBDK-2020)**.

Proyek ini adalah **proyek belajar Software Architecture & Development**: setiap
keputusan penting dicatat, setiap tahap punya bukti selesai yang bisa dicek.

## Mulai membaca dari sini

1. [Visi & kebutuhan](docs/visi.md) — kenapa proyek ini ada dan seperti apa bentuknya.
2. [Architecture Decision Records](docs/adr/README.md) — keputusan dan alasannya.
3. [Panduan setup Windows 10](docs/setup-windows.md) — memasang toolchain.

## Coba sekarang

Buka **<https://masalikovic.github.io/mxm-gameboy/>** di HP. Halaman ini otomatis
diperbarui setiap ada perubahan yang masuk ke `main` dan lulus CI (ADR-0007).

## Build sendiri

```text
cargo test                     # unit test semua crate
cargo run -p gb-headless       # adapter command line
./tools/build-web.sh           # build web ke dist/ (Windows: .\tools\build-web.ps1)
cd dist && python3 -m http.server 8000

./tools/fetch-test-roms.sh     # unduh ROM uji Blargg (dicek SHA-256, tidak di-commit)
./tools/run-test-roms.sh       # jalankan & bandingkan dengan tools/test-roms.tsv

./tools/fetch-gbdk.sh          # unduh GBDK-2020 4.5.0 (dicek SHA-256) ke .gbdk/
./tools/build-homebrew.sh      # build program C di homebrew/ lalu jalankan
```

Di Windows, skrip `.sh` dijalankan dari **Git Bash** (ikut terpasang bersama Git for Windows).
Untuk GBDK di Windows, ikuti [langkah 8 panduan setup](docs/setup-windows.md).

Detail pemasangan alat untuk Windows: [docs/setup-windows.md](docs/setup-windows.md).

## Status

| Tahap | Isi | Status |
|---|---|---|
| M0 | Visi, ADR, setup, kerangka repo, CI, GitHub Pages | ✅ selesai ([jurnal](docs/jurnal/m0.md)) |
| M1 | CPU bagian 1: register, flag, bus, 239/244 opcode | ✅ selesai ([jurnal](docs/jurnal/m1.md)) |
| M2 | CPU lengkap: prefix CB, ROM uji Blargg (10/11 lulus) | ✅ selesai ([jurnal](docs/jurnal/m2.md)) |
| M3 | Timer & interrupt, HALT (11/11 Blargg); homebrew C pertama | ✅ selesai ([jurnal](docs/jurnal/m3.md)) |
| M4 | PPU: layar, tile, V-blank; gambar pertama di HP | 🚧 sedang berjalan |
| M5–M8 | input → cartridge → audio → debugger | belum mulai |

## Legal

Repo ini tidak berisi ROM komersial maupun boot ROM Nintendo. ROM uji publik
diunduh saat tes dan tidak didistribusikan ulang. Lihat [ADR-0006](docs/adr/0006-lisensi.md).

## Lisensi

Kode proyek ini berlisensi **MIT OR Apache-2.0** (lihat `LICENSE-MIT` dan
`LICENSE-APACHE`, ADR-0006).
