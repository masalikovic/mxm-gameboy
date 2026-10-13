# 0007. CI dan deploy: satu workflow, GitHub Pages lewat Actions

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Setiap PR perlu dicek otomatis: format, lint, tes, bukti `gb-core` tanpa `std`
(ADR-0002), dan build WebAssembly (ADR-0003). Setiap perubahan yang masuk ke
`main` perlu langsung bisa dicoba di HP lewat GitHub Pages.

Batasan:

- Repo public, jadi menit GitHub Actions untuk runner standar gratis. Tetap
  dihemat (`docs/visi.md` §5), karena kebiasaan boros terbawa ke proyek private.
- Pemilik belajar dari repo ini, jadi alur CI harus mudah dibaca.
- `wasm-bindgen-cli` harus satu versi dengan crate `wasm-bindgen` (ADR-0003),
  dan compile-nya memakan beberapa menit.

## Pilihan yang dipertimbangkan

1. **Dua workflow** (`ci.yml` untuk PR, `pages.yml` untuk deploy). Mudah dipisah,
   tetapi wasm di-build dua kali setiap push ke `main`.
2. **Deploy dari branch `gh-pages`**: hasil build di-commit ke branch khusus.
   Riwayat git jadi berisi file hasil build, dan butuh izin tulis ke repo.
3. **Satu workflow dengan tiga job** (`check`, `web`, `deploy`). Deploy memakai
   hasil job `web` lewat GitHub Pages sumber "GitHub Actions", tanpa branch tambahan.

## Keputusan

Pilihan 3, di `.github/workflows/ci.yml`:

- `check` dan `web` jalan **paralel** di setiap PR dan setiap push ke `main`.
- `deploy` hanya jalan pada **push ke `main`**, setelah keduanya hijau.
  Halaman yang tampil di HP selalu berasal dari kode yang lulus cek.
- Cache: registry Cargo dan `target/`, serta biner `wasm-bindgen-cli` per versi.
- `concurrency` membatalkan run lama kalau ada push baru ke branch yang sama.
- Versi `wasm-bindgen-cli` ditulis di workflow, lalu `tools/build-web.sh`
  memeriksanya terhadap `Cargo.lock`. Selisih versi menggagalkan build dengan pesan jelas.
- Action resmi GitHub (`actions/*`) dirujuk per versi mayor (`@v4`). Tidak ada
  action pihak ketiga.

## Konsekuensi

- (+) Satu file untuk dibaca; urutan check → deploy terlihat jelas.
- (+) Tidak ada file hasil build di riwayat git.
- (+) Run kedua dan seterusnya jauh lebih cepat berkat cache.
- (−) **Langkah manual sekali saja oleh pemilik repo:** Settings → Pages →
  Build and deployment → Source: **GitHub Actions**. Tanpa itu, job `deploy` gagal.
- (−) PR belum punya halaman preview sendiri. Bisa ditambah kelak bila perlu.
- Pantau: kalau versi `wasm-bindgen` di-upgrade, `WASM_BINDGEN_VERSION` di
  workflow ikut diubah di PR yang sama.
