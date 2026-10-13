# mxm-gameboy

Emulator Game Boy (DMG) dalam **Rust → WebAssembly**, bisa dimainkan di browser HP,
plus program homebrew dalam **C (GBDK-2020)**.

Proyek ini adalah **proyek belajar Software Architecture & Development**: setiap
keputusan penting dicatat, setiap tahap punya bukti selesai yang bisa dicek.

## Mulai membaca dari sini

1. [Visi & kebutuhan](docs/visi.md) — kenapa proyek ini ada dan seperti apa bentuknya.
2. [Architecture Decision Records](docs/adr/README.md) — keputusan dan alasannya.
3. [Panduan setup Windows 10](docs/setup-windows.md) — memasang toolchain.

## Status

| Tahap | Isi | Status |
|---|---|---|
| M0 | Visi, ADR, setup, kerangka repo, CI, GitHub Pages | 🚧 sedang berjalan |
| M1–M8 | CPU → timer → PPU → input → cartridge → audio → debugger | belum mulai |

## Legal

Repo ini tidak berisi ROM komersial maupun boot ROM Nintendo. ROM uji publik
diunduh saat tes dan tidak didistribusikan ulang. Lihat [ADR-0006](docs/adr/0006-lisensi.md).

## Lisensi

Kode proyek ini direncanakan berlisensi **MIT OR Apache-2.0** (ADR-0006);
file lisensi ditambahkan bersama kerangka kode.
