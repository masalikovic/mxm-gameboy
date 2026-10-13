//! Adapter command line untuk `gb-core`.
//!
//! Di M0 program ini hanya mencetak sapaan dari core. Mulai M2, ia akan
//! memuat file ROM uji, menjalankannya tanpa layar, dan melaporkan lulus/gagal
//! (ADR-0005). Berbeda dengan core, adapter boleh memakai `std`: membaca file,
//! menulis ke terminal, dan mengembalikan kode keluar.

fn main() {
    println!("{} (gb-core {})", gb_core::greeting(), gb_core::VERSION);
}
