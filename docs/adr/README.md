# Architecture Decision Records (ADR)

ADR adalah catatan pendek tentang **satu** keputusan arsitektur: konteksnya,
pilihan yang dipertimbangkan, apa yang diputuskan, dan konsekuensinya.
Lihat [ADR-0001](0001-catat-keputusan-dengan-adr.md) untuk alasannya.

## Daftar

| No | Judul | Status |
|---|---|---|
| [0001](0001-catat-keputusan-dengan-adr.md) | Catat keputusan arsitektur dengan ADR | Diterima |
| [0002](0002-struktur-workspace.md) | Struktur workspace: core `no_std` + adapter | Diterima |
| [0003](0003-alat-build-wasm.md) | Build WebAssembly dengan `cargo` + `wasm-bindgen-cli`, tanpa bundler | Diterima |
| [0004](0004-gbdk-2020-untuk-rom-c.md) | GBDK-2020 untuk ROM homebrew dalam C | Diterima |
| [0005](0005-strategi-tes.md) | Strategi tes: unit, ROM uji, golden test | Diterima |
| [0006](0006-lisensi.md) | Lisensi kode & kebijakan ROM | Diterima |
| [0007](0007-ci-dan-deploy.md) | CI dan deploy: satu workflow, GitHub Pages lewat Actions | Diterima |
| [0008](0008-identitas-visual-mxm.md) | Identitas visual: palet MXM | Diterima |
| [0009](0009-cache-busting.md) | Cache busting: folder dist/ dan versi build di URL | Diterima |
| [0010](0010-bentuk-cpu.md) | Bentuk CPU: trait Bus, decode lewat pola bit, siklus dalam T-cycle | Diterima |

## Template

Salin blok ini ke file baru `NNNN-judul-pendek.md`:

```markdown
# NNNN. Judul keputusan

- Status: Diusulkan | Diterima | Digantikan oleh NNNN
- Tanggal: YYYY-MM-DD

## Konteks
Masalah apa yang sedang dihadapi? Batasan apa yang berlaku?

## Pilihan yang dipertimbangkan
1. Pilihan A — plus/minus
2. Pilihan B — plus/minus

## Keputusan
Kita memilih ... karena ...

## Konsekuensi
Apa yang jadi lebih mudah, apa yang jadi lebih sulit, apa yang perlu dipantau.
```

## Aturan

- ADR yang sudah **Diterima** tidak diedit isinya. Kalau keputusan berubah,
  tulis ADR baru dan ubah status yang lama menjadi "Digantikan oleh NNNN".
- Satu ADR = satu keputusan. Kalau judulnya butuh kata "dan", mungkin perlu dua ADR.
