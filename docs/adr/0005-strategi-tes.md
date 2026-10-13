# 0005. Strategi tes: unit, ROM uji, golden test

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Emulator itu benar kalau perilakunya sama dengan perangkat asli. Kita tidak punya
Game Boy asli untuk dibandingkan, jadi "kebenaran" harus diambil dari sumber yang
bisa dipercaya dan dijalankan otomatis. Kita juga ingin CI hemat menit.

## Pilihan yang dipertimbangkan

1. **Hanya mencoba game secara manual** — lambat, tidak bisa diulang, tidak legal (ROM komersial).
2. **Hanya unit test buatan sendiri** — cepat, tetapi kita bisa salah paham spesifikasi
   dan menulis tes yang ikut salah.
3. **Piramida tes berlapis** — unit test untuk detail, ROM uji publik sebagai "hakim
   independen", golden test untuk mencegah regresi.

## Keputusan

Pilihan 3:

| Lapisan | Contoh | Bagaimana dicek | Kapan jalan |
|---|---|---|---|
| **Unit** | `ADD A,B` mengatur flag Z/N/H/C dengan benar | `cargo test` di `gb-core` | Tiap PR |
| **ROM uji publik** | blargg `cpu_instrs`, mooneye, dmg-acid2 | `gb-headless` menjalankan ROM, lalu membaca hasil (lihat bawah) | Tiap PR, setelah komponennya ada |
| **Homebrew** | ROM "Halo" dari C | Teks keluar lewat port serial / isi layar | Mulai M3 |
| **Golden** | Hash framebuffer setelah 300 frame | Dibandingkan dengan hash tersimpan | Mulai M4 |
| **Manual di HP** | Main sebentar, cek suara/sentuhan | Dicatat di jurnal | Akhir tiap tahap |

Cara membaca hasil ROM uji tanpa layar:

- **blargg**: menulis teks hasil ke port serial (register `0xFF01`/`0xFF02`);
  `gb-headless` menangkapnya dan mencari kata "Passed"/"Failed".
- **mooneye**: saat lulus, register CPU berisi deret Fibonacci 3, 5, 8, 13, 21, 34.
- **dmg-acid2**: framebuffer dibandingkan dengan gambar referensi.

ROM uji **tidak di-commit**: skrip `tools/` mengunduh satu rilis yang versinya
dikunci dan diverifikasi dengan SHA-256, lalu di-cache di GitHub Actions (alasan lisensi: ADR-0006).

## Konsekuensi

- (+) Determinisme core (ADR-0002) membuat golden test mungkin — tanpa determinisme, hash akan berubah-ubah.
- (+) ROM uji publik adalah hakim yang tidak bisa kita "akali".
- (−) Butuh frontend headless sejak awal — tetapi memang sudah direncanakan.
- (−) Tes ROM bisa lambat; dijalankan dengan `--release` dan dibatasi jumlah siklus.
- Hemat CI: job dasar (fmt, clippy, test, build wasm) diperkirakan beberapa menit per PR.
  Job tambahan yang lebih berat harus disetujui pemilik repo dulu.
