# Panduan setup — Windows 10

Tujuan panduan ini: dari laptop kosong sampai bisa menjalankan `cargo test` dan
membuka hasil build web di HP. Perkiraan waktu: 30–60 menit (sebagian besar unduhan).

> Kerjakan berurutan. Di akhir tiap langkah ada **cek** — jangan lanjut kalau ceknya gagal.

## 1. Git for Windows

1. Unduh dari <https://git-scm.com/download/win>, jalankan installer.
2. Pilihan default sudah cukup. Saat ditanya editor, pilih yang Anda kenal (mis. VS Code).
3. Setelah selesai, buka **PowerShell** baru dan atur identitas:

```powershell
git config --global user.name "Masalikovic"
git config --global user.email "email-anda@contoh.com"
```

**Cek:** `git --version` menampilkan nomor versi.

## 2. Rust (rustup)

Kenapa rustup: ia mengelola versi compiler Rust dan *target* (mis. WebAssembly).

1. Unduh `rustup-init.exe` dari <https://rustup.rs>.
2. Jalankan. Rust di Windows butuh **linker MSVC** (bagian dari Visual Studio Build Tools).
   Jika installer menawarkan memasang Visual Studio secara otomatis, pilih opsi itu.
   Jika memasang manual: pasang **Build Tools for Visual Studio**, centang workload
   **"Desktop development with C++"** (termasuk Windows SDK).
3. Kembali ke rustup, pilih instalasi default.
4. **Tutup dan buka lagi** PowerShell (supaya `PATH` baru terbaca).

**Cek:**
```powershell
rustc --version
cargo --version
```

## 3. Target WebAssembly & wasm-bindgen

```powershell
rustup target add wasm32-unknown-unknown
rustup component add rustfmt clippy
```

`wasm-bindgen-cli` dipasang **setelah** kerangka kode ada di repo, karena versinya
harus sama persis dengan yang tercatat di `Cargo.lock` (lihat ADR-0003). Perintahnya
akan berbentuk seperti ini (nomor versi diisi nanti):

```powershell
cargo install wasm-bindgen-cli --version <VERSI-DARI-Cargo.lock> --locked
```

**Cek:** `rustup target list --installed` memuat `wasm32-unknown-unknown`.

## 4. Editor: VS Code + rust-analyzer

1. Pasang VS Code dari <https://code.visualstudio.com>.
2. Pasang ekstensi **rust-analyzer** (pemberi saran kode, cek error saat mengetik).
3. Opsional: **Even Better TOML** untuk file `Cargo.toml`.

## 5. Python (hanya untuk server lokal)

Browser tidak mau memuat WebAssembly dari `file://`, jadi kita butuh server HTTP kecil.

1. Pasang Python 3 dari Microsoft Store atau <https://python.org>.

**Cek:** `python --version`.

## 6. Ambil repo & jalankan tes

```powershell
cd $HOME\Documents
git clone https://github.com/Masalikovic/mxm-gameboy.git
cd mxm-gameboy
cargo test
```

**Cek:** keluaran diakhiri `test result: ok`.

## 7. Coba di HP lewat Wi-Fi rumah (opsional — GitHub Pages juga bisa)

1. Build web (skrip akan tersedia setelah kerangka kode ada), lalu:

```powershell
cd web
python -m http.server 8000
```

2. Cari IP laptop: `ipconfig` → baris **IPv4 Address** di adapter Wi-Fi (mis. `192.168.1.23`).
3. Di HP (Wi-Fi yang sama), buka `http://192.168.1.23:8000`.
4. Jika Windows Firewall bertanya, izinkan Python untuk **Private networks** saja.

## Masalah umum

| Gejala | Penyebab & solusi |
|---|---|
| `link.exe not found` saat `cargo build` | Build Tools MSVC belum terpasang / workload C++ belum dicentang. Ulangi langkah 2. |
| `cargo` tidak dikenal | PowerShell lama masih terbuka. Tutup semua, buka baru. |
| HP tidak bisa membuka halaman | Beda jaringan, atau firewall memblokir. Cek langkah 7.4. |
| Antivirus memperlambat build | Tambahkan folder `target\` proyek ke pengecualian antivirus (opsional). |

## Belum diperlukan sekarang

- **GBDK-2020** — dipasang di M3 (panduan menyusul).
- **Node.js/npm** — tidak dipakai (ADR-0003).
