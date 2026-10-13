# Visi & Kebutuhan — mxm-gameboy

> Status: draf M0 · Terakhir diperbarui: 2026-10-13

## 1. Kenapa proyek ini ada

Tujuan utama **bukan** menghasilkan emulator Game Boy terbaik — sudah ada banyak.
Tujuannya adalah **belajar Software Architecture & Development lewat praktik**:
merancang sistem dari nol, mencatat keputusan, memecah pekerjaan, menguji, dan
merilis secara bertahap.

Game Boy (DMG, 1989) dipilih karena:

- **Cukup kecil untuk dipahami utuh** — satu CPU 8-bit, 64 KiB ruang alamat, layar 160×144.
- **Cukup kaya untuk butuh arsitektur** — ada banyak komponen yang saling terkait
  (CPU, bus, timer, interrupt, PPU, APU, cartridge) dan harus disinkronkan per siklus.
- **Dokumentasi & ROM uji publiknya melimpah** — hasil kita bisa *dibuktikan*, bukan dikira-kira.
- **Bisa diuji dari dua sisi** — kita menulis emulatornya (Rust) *dan* program untuknya (C).

## 2. Siapa penggunanya

| Peran | Kebutuhan |
|---|---|
| **Pemilik (pelajar)** | Memahami *kenapa* tiap keputusan diambil; bisa membaca riwayat repo seperti buku pelajaran. |
| **Pemain di HP Android** | Membuka satu link, memilih ROM, bermain dengan tombol sentuh. |
| **Pengembang (kita)** | Tes cepat & deterministik; menambah fitur tanpa merusak yang lama. |

## 3. Lingkup

### Termasuk
- Emulasi **DMG** (Game Boy orisinal, layar hijau 4 warna).
- Frontend **web** (Rust → WebAssembly) yang bisa dimainkan di browser HP, dideploy ke GitHub Pages.
- Frontend **headless** (command line) untuk menjalankan ROM uji di CI.
- Program/game homebrew dalam **C (GBDK-2020)**.
- Cartridge: tanpa MBC, MBC1, MBC3, MBC5 (M6).

### Tidak termasuk (setidaknya untuk sekarang)
- Game Boy Color (CGB), Super Game Boy, link cable.
- Akurasi tingkat *sub-instruction* untuk game-game yang sangat sensitif timing.
- ROM komersial dan boot ROM Nintendo (lihat ADR-0006).
- RTOS untuk STM32 — proyek terpisah, ditunda sampai board tersedia.

## 4. Kebutuhan fungsional (ringkas, per tahap)

| ID | Kebutuhan | Tahap |
|---|---|---|
| F1 | Halaman web "Halo dari emulator" terbuka di HP lewat GitHub Pages | M0 |
| F2 | CPU SM83 menjalankan semua opcode resmi dengan hasil & jumlah siklus yang benar | M1–M2 |
| F3 | Timer (DIV/TIMA) dan interrupt (VBlank, STAT, Timer, Serial, Joypad) | M3 |
| F4 | PPU menggambar background, window, sprite; mode LCD dan STAT | M4–M5 |
| F5 | Input joypad dari tombol sentuh & keyboard | M5 |
| F6 | Cartridge MBC1/3/5; save RAM tersimpan di browser | M6 |
| F7 | Suara 4 kanal via Web Audio | M7 |
| F8 | Debugger/disassembler, save state, PWA offline | M8 |

## 5. Kebutuhan non-fungsional (atribut kualitas)

Inilah yang paling menentukan arsitektur. Setiap atribut diberi cara mengukurnya,
supaya tidak sekadar slogan.

| Atribut | Artinya untuk kita | Cara mengukur |
|---|---|---|
| **Determinisme** | Input yang sama (ROM + urutan tombol) → output yang sama, bit per bit. | Tes "golden": hash framebuffer setelah N frame selalu sama. |
| **Portabilitas** | Core tidak tahu dia berjalan di browser, CLI, atau desktop. | Core dikompilasi tanpa `std` (ADR-0002) — kalau ada I/O, *compiler* menolak. |
| **Testability** | Setiap komponen bisa diuji tanpa layar/suara/file. | Unit test per instruksi; ROM uji dijalankan headless di CI. |
| **Kinerja** | 60 fps stabil di HP Android kelas menengah. | Target: satu frame emulasi < 8 ms di HP (diukur sejak M4). |
| **Keterpahaman** | Kode bisa dibaca sebagai bahan belajar. | Setiap modul punya doc comment "apa & kenapa"; keputusan di ADR. |
| **Legalitas** | Tidak mendistribusikan materi berhak cipta Nintendo/pihak lain. | Tidak ada file ROM di repo; aturan di ADR-0006. |
| **Hemat biaya CI** | Menit GitHub Actions dipakai seperlunya. | CI ringan tiap PR; job berat hanya atas persetujuan. |

## 6. Gambaran arsitektur (tingkat tertinggi)

Gaya yang dipakai adalah **Ports & Adapters** (sering disebut *hexagonal architecture*):
ada satu **core** murni di tengah, dan beberapa **adapter** di pinggir yang
menghubungkannya ke dunia nyata.

```
             ┌──────────────── adapter ────────────────┐
  Browser ── │ gb-web (wasm): canvas, tombol sentuh,   │
  (HP)       │ Web Audio, localStorage                 │──┐
             └─────────────────────────────────────────┘  │
             ┌─────────────────────────────────────────┐  │   ┌───────────────────────┐
  CI / CLI ──│ gb-headless: baca file ROM, jalankan,   │──┼──▶│ gb-core               │
             │ cek hasil tes                           │  │   │ CPU · Bus · Timer ·   │
             └─────────────────────────────────────────┘  │   │ PPU · APU · Cartridge │
             ┌─────────────────────────────────────────┐  │   │ (tanpa I/O, no_std)   │
  (nanti)  ──│ gb-desktop                              │──┘   └───────────────────────┘
             └─────────────────────────────────────────┘
```

**Kenapa begini?** Bagian tersulit (emulasi) hanya ditulis sekali dan diuji tanpa
browser. Browser, CLI, dan desktop hanyalah "colokan" berbeda ke core yang sama.
Kontrak antara keduanya sengaja dibuat kecil, kira-kira:

```text
load_rom(bytes) · set_buttons(state) · run_frame() · framebuffer() · audio_samples()
```

Core tidak pernah membaca jam, file, atau angka acak — waktu di dalam core
adalah **jumlah siklus CPU**. Frontend-lah yang memutuskan kapan `run_frame()` dipanggil.

## 7. Risiko utama

| Risiko | Dampak | Mitigasi |
|---|---|---|
| Timing PPU/CPU rumit, mudah salah | Grafis rusak, game macet | ROM uji per komponen; mulai dari model sederhana, ukur dengan dmg-acid2 |
| Kinerja wasm di HP kurang | Game patah-patah | Ukur sejak awal (M4); hindari alokasi per frame |
| Alat build wasm berubah (ekosistem bergeser) | Build rusak | Pakai alat paling dasar & versi dikunci (ADR-0003) |
| Lisensi ROM uji tidak jelas | Masalah hukum | Tidak di-commit, diunduh saat tes (ADR-0006) |
| Lingkup melebar | Tidak pernah selesai | Tahapan M0–M8 dengan "bukti selesai" yang jelas |

## 8. Definisi "selesai" untuk M0

- [x] Dokumen ini dan ADR 0001–0006 ada di `main`.
- [x] Panduan setup Windows 10 bisa diikuti sampai `cargo test` jalan di laptop.
      (Diverifikasi pemilik di laptop Windows 10, 2026-10-13: 3 tes lulus;
      `build-web.ps1` + server lokal juga dibuka dari HP lewat Wi-Fi.)
- [x] Kerangka workspace Rust ter-compile; `cargo test` hijau di CI.
- [x] Link GitHub Pages menampilkan **"Halo dari emulator"** — teks itu berasal dari
      fungsi Rust yang dipanggil lewat WebAssembly — dan terbuka di HP Android.
      (Diverifikasi pemilik di HP, 2026-10-13.)
