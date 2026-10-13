//! Adapter command line untuk `gb-core`.
//!
//! ```text
//! gb-headless                        mencetak sapaan (smoke test M0)
//! gb-headless <rom.gb> [maks-siklus] menjalankan ROM, menilai keluaran serial
//! gb-headless <rom.gb> --frames N [--screenshot layar.pgm]
//!                                    menjalankan N frame, mencetak hash layar
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
    if args.get(1).map(String::as_str) == Some("--frames") {
        return screen_mode(path, &args[2..]);
    }
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

/// Mode layar: jalankan N frame, cetak hash framebuffer, dan (opsional)
/// simpan gambar PGM. Hash dipakai golden test (ADR-0005): layar yang sama
/// persis selalu menghasilkan hash yang sama, karena core deterministik.
fn screen_mode(path: &str, args: &[String]) -> ExitCode {
    let Some(Ok(frames)) = args.first().map(|s| s.parse::<u32>()) else {
        eprintln!("Pakai: gb-headless <rom.gb> --frames N [--screenshot layar.pgm]");
        return ExitCode::from(2);
    };
    let screenshot = match args.get(1).map(String::as_str) {
        Some("--screenshot") => args.get(2),
        _ => None,
    };
    let rom = match std::fs::read(path) {
        Ok(rom) => rom,
        Err(error) => {
            eprintln!("Gagal membaca {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let mut gb = GameBoy::new(rom.into_boxed_slice());
    for _ in 0..frames {
        if let Err(error) = gb.run_frame() {
            eprintln!("Berhenti: {error:?}");
            return ExitCode::from(2);
        }
    }
    println!("{:016x}", fnv1a64(gb.framebuffer()));
    if let Some(file) = screenshot
        && let Err(error) = std::fs::write(file, to_pgm(gb.framebuffer()))
    {
        eprintln!("Gagal menulis {file}: {error}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

/// Hash FNV-1a 64-bit: sederhana, cukup untuk mendeteksi perubahan layar.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Framebuffer → gambar PGM (format abu-abu paling sederhana).
/// Tingkat 0 (terang) … 3 (gelap) → putih … hitam.
fn to_pgm(framebuffer: &[u8]) -> Vec<u8> {
    let mut image = format!(
        "P5\n{} {}\n255\n",
        gb_core::ppu::WIDTH,
        gb_core::ppu::HEIGHT
    )
    .into_bytes();
    image.extend(
        framebuffer
            .iter()
            .map(|&shade| [255, 170, 85, 0][usize::from(shade & 3)]),
    );
    image
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
