//! Adapter command line untuk `gb-core`.
//!
//! ```text
//! gb-headless                       mencetak sapaan (smoke test M0)
//! gb-headless <rom.gb> [maks-siklus] menjalankan ROM tanpa layar
//! ```
//!
//! Saat menjalankan ROM, semua byte yang dikirim lewat port serial dicetak ke
//! terminal. ROM uji Blargg mencetak hasilnya lewat jalur itu, diakhiri
//! "Passed" atau "Failed" (ADR-0005). Kode keluar:
//!
//! - `0`: keluaran serial memuat "Passed";
//! - `1`: keluaran serial memuat "Failed";
//! - `2`: error lain (file tidak ada, opcode belum ditulis, batas siklus habis).
//!
//! Berbeda dengan core, adapter boleh memakai `std`: membaca file, menulis ke
//! terminal, dan mengembalikan kode keluar.

use std::io::Write;
use std::process::ExitCode;

use gb_core::machine::GameBoy;

/// Batas bawaan: ±60 detik waktu Game Boy (4.194.304 siklus per detik).
const DEFAULT_MAX_CYCLES: u64 = 60 * 4_194_304;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        println!("{} (gb-core {})", gb_core::greeting(), gb_core::VERSION);
        return ExitCode::SUCCESS;
    };
    let max_cycles = match args.get(1).map(|s| s.parse::<u64>()) {
        None => DEFAULT_MAX_CYCLES,
        Some(Ok(n)) => n,
        Some(Err(_)) => {
            eprintln!("maks-siklus harus berupa angka: {}", args[1]);
            return ExitCode::from(2);
        }
    };
    let rom = match std::fs::read(path) {
        Ok(rom) => rom,
        Err(error) => {
            eprintln!("Gagal membaca {path}: {error}");
            return ExitCode::from(2);
        }
    };
    run(GameBoy::new(rom.into_boxed_slice()), max_cycles)
}

/// Menjalankan ROM sampai keluaran serial memuat "Passed"/"Failed", CPU
/// bertemu opcode yang belum ada, atau batas siklus habis.
fn run(mut gb: GameBoy, max_cycles: u64) -> ExitCode {
    let mut output = Vec::new();
    let mut cycles: u64 = 0;
    let mut stdout = std::io::stdout();
    while cycles < max_cycles {
        match gb.step() {
            Ok(n) => cycles += u64::from(n),
            Err(error) => {
                eprintln!("\nBerhenti setelah {cycles} siklus: {error:?}");
                return ExitCode::from(2);
            }
        }
        if let Some(byte) = gb.bus.take_serial_byte() {
            output.push(byte);
            // Kegagalan menulis ke terminal tidak boleh menghentikan tes.
            let _ = stdout.write_all(&[byte]).and_then(|()| stdout.flush());
            if output.ends_with(b"Passed") {
                println!();
                return ExitCode::SUCCESS;
            }
            if output.ends_with(b"Failed") {
                println!();
                return ExitCode::from(1);
            }
        }
    }
    eprintln!("\nBatas {max_cycles} siklus habis tanpa \"Passed\" atau \"Failed\".");
    ExitCode::from(2)
}
