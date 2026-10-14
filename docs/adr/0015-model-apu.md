# 0015. Model APU: sampel dibuat di core, diputar oleh adapter

- Status: Diterima
- Tanggal: 2026-10-14

## Konteks

M7 menambah suara (`visi.md` F7). APU Game Boy punya empat kanal: dua
gelombang kotak (kanal 1 dengan *sweep* frekuensi), satu gelombang bebas dari
RAM 16 byte, dan satu *noise* dari LFSR. Kanal diatur lewat register
NR10–NR52; panjang bunyi, *envelope* volume, dan *sweep* digerakkan oleh
*frame sequencer* 512 Hz yang di DMG dipicu bit 12 pencacah DIV.

Core harus tetap `no_std` dan deterministik (ADR-0002, ADR-0011), padahal
suara akhirnya diputar oleh sesuatu yang berjalan menurut jam dinding
(Web Audio di browser). Yang perlu diputuskan: di mana gelombang dibuat,
dalam bentuk apa diserahkan, dan seberapa teliti.

## Pilihan yang dipertimbangkan

**Di mana gelombang dibuat**

1. **Core membuat sampel PCM** pada laju yang diminta adapter (mis. 48 kHz),
   dalam satuan siklus CPU. Adapter hanya memutar.
2. Core hanya menyerahkan register; adapter membuat suara dengan oscillator
   Web Audio. Ringan, tetapi perilaku APU (length, sweep, LFSR) pindah ke
   JavaScript, tidak bisa dites dengan ROM uji, dan tidak dibawa adapter lain.

**Ketelitian**

1. **Kanal dimajukan per instruksi** (seperti PPU dan timer), sampel diambil
   langsung dari keadaan kanal (*point sampling*). Sederhana dan cukup cepat.
2. Per siklus dengan filter *band-limited*: paling bersih, jauh lebih berat
   untuk HP.

## Keputusan

- **Core membuat sampel** (`apu.rs`), stereo `f32` berselang-seling, pada
  laju yang dipasang adapter. Laju 0 (bawaan) = tidak membuat sampel sama
  sekali, supaya `gb-headless` dan tes tidak menumpuk memori.
- Waktu sampel dihitung dengan akumulator bilangan bulat dalam siklus CPU
  (4.194.304 Hz), bukan jam dinding: dua kali jalan = sampel yang sama persis.
- **Frame sequencer** dipicu bus saat bit 12 pencacah timer jatuh 1 → 0,
  termasuk saat DIV ditulis (perilaku DMG yang diuji Blargg).
- **Point sampling** + filter *high-pass* sederhana (meniru kapasitor
  keluaran DMG) agar tidak ada DC offset. Aliasing nada tinggi diterima.
- Register suara pindah dari tabel I/O umum ke `Apu`, termasuk bit yang
  terbaca 1 dan aturan "mati = register dikosongkan" (NR52).
- Perilaku akses RAM gelombang **saat kanal 3 berbunyi** (yang bergantung
  pada siklus persis di dalam instruksi) tidak ditiru; tiga ROM uji Blargg
  terkait (09, 10, 12) dicatat sebagai batasan.

## Konsekuensi

- (+) Seluruh perilaku APU di Rust, dites dengan ROM Blargg `dmg_sound`.
- (+) Adapter lain (kelak cyberdeck) mendapat sampel yang sama.
- (−) Membuat sampel memakan CPU HP; harus diukur (target tetap < 8 ms/frame).
- (−) Point sampling bisa terdengar "kasar" di nada sangat tinggi.
- Pantau: kalau waktu per frame di HP naik terlalu banyak, pindah ke
  pembuatan sampel per blok, bukan per instruksi.
