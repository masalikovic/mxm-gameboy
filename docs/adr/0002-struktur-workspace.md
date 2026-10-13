# 0002. Struktur workspace: core `no_std` + adapter

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Emulator akan dijalankan di setidaknya tiga tempat: browser (wasm), CI (headless),
dan nanti desktop. Logika emulasinya sama, tetapi cara menampilkan gambar, memutar
suara, membaca tombol, dan menyimpan file berbeda di tiap tempat.

Atribut kualitas yang paling penting (lihat `docs/visi.md`):
**determinisme**, **portabilitas**, dan **testability**.

## Pilihan yang dipertimbangkan

1. **Satu crate** berisi emulator + kode web.
   Sederhana, tetapi kode browser (`wasm-bindgen`, `web-sys`) akan merembes ke logika
   emulasi, dan tes harus berurusan dengan browser.
2. **Workspace Cargo multi-crate, core memakai `std`**, dengan aturan "jangan pakai I/O
   di core" yang dijaga lewat review.
3. **Workspace Cargo multi-crate, core `#![no_std]`** (+ `alloc`). Aturan "tanpa I/O"
   dijaga oleh *compiler*: tanpa `std`, tidak ada `std::fs`, `std::time`, `std::net`,
   atau `println!`.

## Keputusan

Pilihan 3, dengan struktur:

```
mxm-gameboy/
├─ Cargo.toml            # workspace
├─ crates/
│  ├─ gb-core/           # lib, #![no_std] + alloc, tanpa dependensi eksternal
│  ├─ gb-web/            # cdylib → wasm; adapter browser (wasm-bindgen)
│  └─ gb-headless/       # bin; adapter CLI untuk menjalankan ROM uji
├─ web/                  # HTML/CSS/JS statis untuk GitHub Pages
├─ homebrew/             # program C (GBDK-2020) — mulai M3
├─ tools/                # skrip bantu (mis. unduh ROM uji)
└─ docs/                 # visi, ADR, panduan, jurnal
```

Aturan dependensi (panah = "boleh bergantung pada"):

```
gb-web ──▶ gb-core ◀── gb-headless
```

`gb-core` tidak boleh bergantung pada crate lain di workspace, dan sebisa mungkin
tidak pada crate eksternal apa pun.

Prinsip di dalam core:

- **Waktu = siklus CPU.** Tidak ada jam dinding. Frontend memanggil `run_frame()`.
- **Tidak ada keacakan.** Isi RAM awal ditentukan (nol), bukan acak.
- **Input masuk, output keluar lewat API**, bukan lewat efek samping.

## Konsekuensi

- (+) Kalau seseorang (atau kita) tidak sengaja memakai I/O di core, build gagal.
  Ini *arsitektur yang ditegakkan oleh alat*, bukan sekadar niat.
- (+) Tes core cepat — tidak ada browser, tidak ada file.
- (+) Core yang `no_std` nantinya bisa dicoba di mikrokontroler — nyambung dengan
  rencana belajar embedded (STM32).
- (−) Sedikit lebih repot: `Vec`/`Box` diimpor dari `alloc`, bukan `std`;
  tes unit memakai `#[cfg(test)] extern crate std;`.
- (−) Tiga crate untuk proyek yang awalnya kosong terasa berlebihan. Kita terima,
  karena struktur ini mahal untuk diubah belakangan dan murah untuk dibuat sekarang.
