# 0017. PWA offline: service worker dengan cache per versi build

- Status: Diterima
- Tanggal: 2026-10-14

## Konteks

M8 menutup F8 di `visi.md`: emulator bisa dipasang di layar utama HP dan
dibuka tanpa internet. Itu butuh manifest PWA dan service worker yang
menyimpan halaman, JS, `.wasm`, dan ROM demo.

ADR-0009 sudah membuat setiap URL aset memuat versi build (`?v=…`) supaya
browser HP tidak memakai berkas lama. Cache service worker harus bekerja
sama dengan aturan itu, bukan melawannya. Service worker hanya boleh
dipasang di https atau localhost.

## Pilihan yang dipertimbangkan

1. **Cache dulu untuk semuanya**: paling cepat, tetapi halaman lama bisa
   terus dilayani walau sudah ada build baru.
2. **Jaringan dulu untuk semuanya**: selalu baru, tetapi setiap aset
   menunggu jaringan dulu sebelum jatuh ke cache.
3. **Campuran**: halaman (navigasi) jaringan dulu; aset lain cache dulu —
   aman karena URL aset sudah memuat versi build, jadi isinya tidak pernah
   berubah untuk URL yang sama.
4. Pustaka (Workbox): lengkap, tetapi menambah dependensi dan langkah build
   untuk kebutuhan sekecil ini.

## Keputusan

- **Pilihan 3**, ditulis tangan di `web/sw.js` (±60 baris).
- Nama cache = `mxm-gameboy-<versi build>`. Daftar berkas (termasuk ROM
  demo di `dist/roms`) diisi oleh `tools/build-web.sh`/`.ps1`, sama seperti
  stempel `__BUILD__`.
- `skipWaiting` + `clients.claim`: build baru dipakai pada kunjungan
  berikutnya tanpa menunggu semua tab ditutup; cache build lama dihapus saat
  service worker baru aktif.
- Didaftarkan hanya kalau `isSecureContext`. Di `http://192.168.x.x`
  (laptop → HP, M0) halaman tetap jalan, hanya tanpa offline.
- Ikon dibuat dari kode (`tools/make-icons.py`), bukan gambar pihak ketiga
  (ADR-0006), dengan isi penting di dalam zona aman ikon *maskable*.

## Konsekuensi

- (+) Sekali dibuka online, emulator, demo, simpan permainan, dan save state
  semuanya jalan offline.
- (+) Tidak ada dependensi baru; aturan versi ADR-0009 tetap satu-satunya
  sumber "berkas mana yang baru".
- (−) Kunjungan pertama setelah build baru masih memakai halaman baru dari
  jaringan, tetapi cache baru baru lengkap beberapa detik kemudian; offline
  tepat di detik itu memakai build sebelumnya.
- (−) ROM yang dimuat pengguna tidak ikut di-cache (memang tidak pernah
  disimpan; ADR-0006). Pengguna memilih filenya lagi.
- Pantau: kalau aset bertambah besar, `cache.addAll` saat instalasi bisa
  lama di jaringan HP yang lambat.
