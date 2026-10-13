//! Inti emulator Game Boy (DMG).
//!
//! Crate ini adalah "core" dalam arsitektur ports & adapters (lihat
//! `docs/visi.md` §6 dan ADR-0002). Aturannya:
//!
//! - **`#![no_std]`** — tidak ada file, jam, jaringan, atau `println!`. Kalau ada
//!   yang mencoba memakainya, build gagal. Itu disengaja: aturan arsitektur
//!   dijaga oleh compiler, bukan hanya oleh niat baik.
//! - **Tanpa `unsafe`** — dikunci dengan `#![forbid(unsafe_code)]`.
//! - **Deterministik** — input yang sama selalu menghasilkan output yang sama.
//!   Waktu di dalam core diukur dalam siklus CPU, bukan jam dinding.
//!
//! Modul:
//!
//! - [`bus`]: cara CPU membaca dan menulis memori (ADR-0010).
//! - [`cpu`]: CPU SM83, dibangun instruksi demi instruksi di M1–M2.
//! - [`machine`]: [`machine::GameBoy`], yaitu CPU + bus memori lengkap.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod bus;
pub mod cpu;
pub mod machine;

/// Versi `gb-core`, diambil dari `Cargo.toml` saat compile.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sapaan dari core.
///
/// Halaman web menampilkan teks ini lewat WebAssembly, dan `gb-headless`
/// mencetaknya ke terminal. Kalau teks yang sama muncul di keduanya, berarti
/// kedua adapter benar-benar memakai core yang sama.
#[must_use]
pub const fn greeting() -> &'static str {
    "Halo dari emulator"
}

#[cfg(test)]
mod tests {
    use super::{VERSION, greeting};

    #[test]
    fn greeting_is_the_m0_text() {
        assert_eq!(greeting(), "Halo dari emulator");
    }

    #[test]
    fn version_comes_from_cargo_toml() {
        assert_eq!(VERSION, "0.1.0");
    }
}
