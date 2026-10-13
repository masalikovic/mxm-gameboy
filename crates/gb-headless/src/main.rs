//! Adapter command line untuk `gb-core`.
//!
//! ```text
//! gb-headless                        mencetak sapaan (smoke test M0)
//! gb-headless <rom.gb> [maks-siklus] menjalankan ROM, menilai keluaran serial
//! gb-headless <rom.gb> --frames N [--hold right,a] [--screenshot layar.pgm]
//!                                    menjalankan N frame, mencetak hash layar
//! ```
//!
//! `--hold` menahan tombol selama semua frame (right, left, up, down, a, b,
//! select, start). Dengan begitu golden test bisa membuktikan bahwa input
//! benar-benar sampai ke game, tanpa manusia menekan tombol.
//!
//! Saat menjalankan ROM, semua byte yang dikirim lewat port serial dicetak ke
//! terminal. Dua konvensi ROM uji dikenali (ADR-0005):
//!
//! - Blargg mencetak teks yang diakhiri "Passed" atau "Failed";
//! - Mooneye mengirim deret Fibonacci 3, 5, 8, 13, 21, 34 kalau lulus, atau
//!   enam byte 0x42 kalau gagal.
//!
//! Kode keluar:
//!
//! - `0`: lulus ("Passed" atau deret Fibonacci);
//! - `1`: gagal ("Failed" atau 0x42 ×6);
//! - `2`: error lain (file tidak ada, opcode belum ditulis, batas siklus habis).
//!
//! Berbeda dengan core, adapter boleh memakai `std`: membaca file, menulis ke
//! terminal, dan mengembalikan kode keluar.

use std::io::Write;
use std::process::ExitCode;

use gb_core::joypad::{Button, Buttons};
use gb_core::machine::GameBoy;

/// Tanda lulus/gagal ROM uji Mooneye lewat serial.
const MOONEYE_PASS: [u8; 6] = [3, 5, 8, 13, 21, 34];
const MOONEYE_FAIL: [u8; 6] = [0x42; 6];

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
    match GameBoy::new(rom.into_boxed_slice()) {
        Ok(gb) => run(gb, max_cycles),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

/// Mode layar: jalankan N frame, cetak hash framebuffer, dan (opsional)
/// simpan gambar PGM. Hash dipakai golden test (ADR-0005): layar yang sama
/// persis selalu menghasilkan hash yang sama, karena core deterministik.
fn screen_mode(path: &str, args: &[String]) -> ExitCode {
    const USAGE: &str =
        "Pakai: gb-headless <rom.gb> --frames N [--hold right,a] [--screenshot layar.pgm]";
    let Some(Ok(frames)) = args.first().map(|s| s.parse::<u32>()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut screenshot = None;
    let mut held = Buttons(0);
    let mut rest = args[1..].iter();
    while let Some(flag) = rest.next() {
        match (flag.as_str(), rest.next()) {
            ("--screenshot", Some(file)) => screenshot = Some(file),
            ("--hold", Some(names)) => match parse_buttons(names) {
                Ok(buttons) => held = buttons,
                Err(name) => {
                    eprintln!("Tombol tidak dikenal: {name}");
                    return ExitCode::from(2);
                }
            },
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let rom = match std::fs::read(path) {
        Ok(rom) => rom,
        Err(error) => {
            eprintln!("Gagal membaca {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let mut gb = match GameBoy::new(rom.into_boxed_slice()) {
        Ok(gb) => gb,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    gb.set_buttons(held);
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

/// "right,a" → `Buttons` dengan bit kanan dan A. Nama yang salah dikembalikan
/// sebagai error supaya salah ketik di `screen.hash` tidak lolos diam-diam.
fn parse_buttons(names: &str) -> Result<Buttons, &str> {
    names.split(',').try_fold(Buttons(0), |buttons, name| {
        let button = match name {
            "right" => Button::Right,
            "left" => Button::Left,
            "up" => Button::Up,
            "down" => Button::Down,
            "a" => Button::A,
            "b" => Button::B,
            "select" => Button::Select,
            "start" => Button::Start,
            _ => return Err(name),
        };
        Ok(Buttons(buttons.0 | button as u8))
    })
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
            if output.ends_with(b"Passed") || output.ends_with(&MOONEYE_PASS) {
                println!();
                return ExitCode::SUCCESS;
            }
            if output.ends_with(b"Failed") || output.ends_with(&MOONEYE_FAIL) {
                println!();
                return ExitCode::from(1);
            }
        }
    }
    eprintln!("\nBatas {max_cycles} siklus habis tanpa tanda lulus atau gagal.");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::parse_buttons;

    #[test]
    fn button_names_become_bits() {
        assert_eq!(parse_buttons("right").map(|b| b.0), Ok(0x01));
        assert_eq!(
            parse_buttons("up,a,start").map(|b| b.0),
            Ok(0x04 | 0x10 | 0x80)
        );
        assert_eq!(parse_buttons("left,kanan").map(|b| b.0), Err("kanan"));
    }
}
