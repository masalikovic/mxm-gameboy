# 0001. Catat keputusan arsitektur dengan ADR

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Proyek ini dibuat untuk belajar. Kode menjelaskan *apa* yang dilakukan sistem,
tetapi jarang menjelaskan *kenapa* sistem dibentuk seperti itu, dan pilihan lain
apa yang ditolak. Tiga bulan lagi, alasan itu akan terlupa — baik oleh pemilik
repo maupun oleh sesi Claude berikutnya, yang mulai tanpa ingatan percakapan ini.

## Pilihan yang dipertimbangkan

1. **Tidak mencatat** — paling cepat sekarang, paling mahal nanti.
2. **Wiki GitHub / dokumen terpisah** — mudah ditulis, tetapi tidak ikut ter-review
   di PR dan bisa tidak sinkron dengan kode.
3. **ADR berupa Markdown di repo** (`docs/adr/`), format pendek gaya Michael Nygard.

## Keputusan

Pilihan 3. Setiap keputusan yang sulit diubah atau berdampak ke banyak modul
ditulis sebagai ADR bernomor, dan masuk lewat PR yang sama dengan perubahan kodenya.

## Konsekuensi

- (+) Riwayat keputusan ikut versi kode; bisa dibaca dengan `git log docs/adr`.
- (+) Memaksa kita menulis pilihan alternatif — latihan berpikir arsitektur.
- (+) Sesi Claude baru bisa membaca ADR untuk memahami konteks proyek.
- (−) Butuh disiplin; keputusan kecil (nama variabel, format kode) *tidak* perlu ADR.
