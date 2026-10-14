//! Adapter command line untuk `gb-core`.
//!
//! ```text
//! gb-headless                        mencetak sapaan (smoke test M0)
//! gb-headless <rom.gb> [maks-siklus] menjalankan ROM, menilai keluaran serial
//! gb-headless <rom.gb> --frames N [--hold right,a] [--screenshot layar.pgm]
//!              [--audio] [--wav suara.wav] [--state-check M]
//!                                    menjalankan N frame, mencetak hash layar
//!                                    (atau hash suara dengan --audio)
//! ```
//!
//! `--state-check M` menguji save state (ADR-0016): setelah N frame state
//! disimpan; mesin asli lanjut M frame, mesin baru memuat state lalu juga
//! jalan M frame. Seluruh state keduanya harus sama persis (kode keluar 1
//! kalau tidak).
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
//!   enam byte 0x42 kalau gagal;
//! - Blargg `dmg_sound` menulis ke RAM cartridge: tanda `DE B0 61` di
//!   A001–A003, status di A000 (0x80 = masih berjalan, 0x00 = lulus, lainnya
//!   = gagal), dan teks hasil mulai A004.
//!
//! Kode keluar:
//!
//! - `0`: lulus ("Passed", deret Fibonacci, atau status RAM 0x00);
//! - `1`: gagal ("Failed", 0x42 ×6, atau status RAM selain 0x00/0x80);
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

/// Tanda protokol memori Blargg di RAM cartridge (A001–A003).
const BLARGG_SIGNATURE: [u8; 3] = [0xDE, 0xB0, 0x61];
/// RAM diperiksa tiap sekian siklus (±1 frame), bukan tiap instruksi.
const MEMORY_CHECK_CYCLES: u64 = 65_536;

/// Hasil protokol memori Blargg, kalau tesnya sudah selesai:
/// `Some((lulus, teks))`. `None` kalau tanda belum ada atau masih berjalan.
fn blargg_memory_result(ram: &[u8]) -> Option<(bool, String)> {
    let (status, rest) = ram.split_first()?;
    if rest.get(..3)? != BLARGG_SIGNATURE || *status == 0x80 {
        return None;
    }
    let text: String = rest[3..]
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| char::from(byte))
        .collect();
    Some((*status == 0, text))
}

/// Laju sampel suara untuk `--audio` dan `--wav` (tetap, supaya hash stabil).
const AUDIO_RATE: u32 = 48_000;

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
    const USAGE: &str = "Pakai: gb-headless <rom.gb> --frames N [--hold right,a] \
                         [--screenshot layar.pgm] [--audio] [--wav suara.wav] \
                         [--state-check M]";
    let Some(Ok(frames)) = args.first().map(|s| s.parse::<u32>()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut screenshot = None;
    let mut wav = None;
    let mut audio = false;
    let mut state_check: Option<u32> = None;
    let mut held = Buttons(0);
    let mut rest = args[1..].iter();
    while let Some(flag) = rest.next() {
        if flag == "--audio" {
            audio = true;
            continue;
        }
        match (flag.as_str(), rest.next()) {
            ("--wav", Some(file)) => wav = Some(file),
            ("--state-check", Some(more)) => {
                let Ok(more) = more.parse() else {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                };
                state_check = Some(more);
            }
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
    let mut gb = match GameBoy::new(rom.clone().into_boxed_slice()) {
        Ok(gb) => gb,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    gb.set_buttons(held);
    if let Some(more) = state_check {
        return state_roundtrip(gb, &rom, held, frames, more);
    }
    let record = audio || wav.is_some();
    if record {
        gb.set_sample_rate(AUDIO_RATE);
    }
    // Sampel dibulatkan ke 16-bit: itu yang disimpan ke WAV dan di-hash.
    let mut pcm: Vec<i16> = Vec::new();
    for _ in 0..frames {
        if let Err(error) = gb.run_frame() {
            eprintln!("Berhenti: {error:?}");
            return ExitCode::from(2);
        }
        if record {
            pcm.extend(gb.take_samples().into_iter().map(to_i16));
        }
    }
    if audio {
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        println!("{:016x}", fnv1a64(&bytes));
    } else {
        println!("{:016x}", fnv1a64(gb.framebuffer()));
    }
    if let Some(file) = wav
        && let Err(error) = std::fs::write(file, to_wav(&pcm))
    {
        eprintln!("Gagal menulis {file}: {error}");
        return ExitCode::from(2);
    }
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

/// Uji save state: simpan setelah `frames`, lalu bandingkan mesin asli dan
/// mesin yang dimuat ulang setelah keduanya jalan `more` frame lagi.
fn state_roundtrip(mut gb: GameBoy, rom: &[u8], held: Buttons, frames: u32, more: u32) -> ExitCode {
    let run = |gb: &mut GameBoy, count: u32| -> bool { (0..count).all(|_| gb.run_frame().is_ok()) };
    if !run(&mut gb, frames) {
        eprintln!("Berhenti sebelum state disimpan");
        return ExitCode::from(2);
    }
    let snapshot = gb.save_state();
    let Ok(mut restored) = GameBoy::new(rom.to_vec().into_boxed_slice()) else {
        return ExitCode::from(2);
    };
    restored.set_buttons(held);
    if let Err(error) = restored.load_state(&snapshot) {
        eprintln!("Gagal memuat state: {error}");
        return ExitCode::from(1);
    }
    if !run(&mut gb, more) || !run(&mut restored, more) {
        eprintln!("Berhenti setelah state dimuat");
        return ExitCode::from(2);
    }
    let (a, b) = (gb.save_state(), restored.save_state());
    if a == b {
        println!(
            "state cocok ({} byte, {:016x})",
            snapshot.len(),
            fnv1a64(&a)
        );
        ExitCode::SUCCESS
    } else {
        let first = a
            .iter()
            .zip(&b)
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        println!(
            "STATE BERBEDA mulai byte {first} (dari {} / {})",
            a.len(),
            b.len()
        );
        ExitCode::from(1)
    }
}

/// Sampel −1.0 … +1.0 → bilangan bulat 16-bit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "nilai sudah dijepit ke −32767 … 32767 sebelum diubah"
)]
fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// Sampel 16-bit stereo → berkas WAV (format PCM paling sederhana).
fn to_wav(pcm: &[i16]) -> Vec<u8> {
    let data_len = u32::try_from(pcm.len() * 2).unwrap_or(u32::MAX);
    let mut wav = Vec::with_capacity(44 + pcm.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // panjang bagian fmt
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&2u16.to_le_bytes()); // stereo
    wav.extend_from_slice(&AUDIO_RATE.to_le_bytes());
    wav.extend_from_slice(&(AUDIO_RATE * 4).to_le_bytes()); // byte per detik
    wav.extend_from_slice(&4u16.to_le_bytes()); // byte per sampel stereo
    wav.extend_from_slice(&16u16.to_le_bytes()); // bit per sampel
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in pcm {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
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
    let mut next_memory_check = MEMORY_CHECK_CYCLES;
    while cycles < max_cycles {
        match gb.step() {
            Ok(n) => cycles += u64::from(n),
            Err(error) => {
                eprintln!("\nBerhenti setelah {cycles} siklus: {error:?}");
                return ExitCode::from(2);
            }
        }
        if cycles >= next_memory_check {
            next_memory_check += MEMORY_CHECK_CYCLES;
            if let Some((passed, text)) = blargg_memory_result(gb.bus.cartridge().ram()) {
                println!("{}", text.trim_end());
                return if passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                };
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
    use super::{blargg_memory_result, parse_buttons, to_i16, to_wav};

    #[test]
    fn samples_become_16_bit_wav() {
        assert_eq!(to_i16(1.0), 32767);
        assert_eq!(to_i16(-2.0), -32767, "dijepit");
        let wav = to_wav(&[1, -1]);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 44 + 4);
        assert_eq!(&wav[44..], &[1, 0, 0xFF, 0xFF]);
    }

    #[test]
    fn blargg_memory_protocol() {
        let mut ram = vec![0u8; 16];
        assert_eq!(blargg_memory_result(&ram), None, "belum ada tanda");
        ram[..4].copy_from_slice(&[0x80, 0xDE, 0xB0, 0x61]);
        assert_eq!(blargg_memory_result(&ram), None, "masih berjalan");
        ram[0] = 0x00;
        ram[4..10].copy_from_slice(b"Passed");
        assert_eq!(blargg_memory_result(&ram), Some((true, "Passed".into())));
        ram[0] = 0x03;
        assert_eq!(blargg_memory_result(&ram).map(|r| r.0), Some(false));
        assert_eq!(blargg_memory_result(&[]), None, "tanpa RAM");
    }

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
