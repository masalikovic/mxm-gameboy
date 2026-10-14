use alloc::boxed::Box;
use alloc::vec;

use super::{StateError, VERSION};
use crate::machine::GameBoy;

/// ROM kecil yang menyentuh banyak komponen: menyalakan suara (kanal 2),
/// lalu berputar sambil menaikkan byte di C000 dan menulis ke VRAM.
fn rom() -> Box<[u8]> {
    let program = [
        0x3E, 0x80, 0xE0, 0x26, // LD A,$80 / LDH ($26),A   APU menyala
        0x3E, 0xF0, 0xE0, 0x17, // LD A,$F0 / LDH ($17),A   volume kanal 2
        0x3E, 0x80, 0xE0, 0x16, // LD A,$80 / LDH ($16),A   duty 50 %
        0x3E, 0x87, 0xE0, 0x19, // LD A,$87 / LDH ($19),A   trigger
        0x21, 0x00, 0xC0, //       LD HL,$C000
        0x34, //                   loop: INC (HL)
        0x7E, //                   LD A,(HL)
        0xEA, 0x00, 0x98, //       LD ($9800),A            tile peta BG
        0x18, 0xF9, //             JR loop
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x100 + program.len()].copy_from_slice(&program);
    rom.into_boxed_slice()
}

fn run(gb: &mut GameBoy, frames: u32) {
    for _ in 0..frames {
        gb.run_frame().expect("tidak ada opcode yang hilang");
    }
}

/// Inti bukti: mesin yang dimuat dari state berjalan *persis* sama dengan
/// mesin aslinya — seluruh state dibandingkan byte demi byte, bukan hanya
/// layarnya. Field yang lupa disimpan membuat keduanya menyimpang.
#[test]
fn loaded_machine_continues_exactly_like_the_original() {
    let mut original = GameBoy::new(rom()).expect("ROM valid");
    run(&mut original, 30);
    let snapshot = original.save_state();
    run(&mut original, 30);

    let mut restored = GameBoy::new(rom()).expect("ROM valid");
    restored.load_state(&snapshot).expect("state valid");
    run(&mut restored, 30);

    assert_eq!(restored.save_state(), original.save_state());
    assert_eq!(restored.framebuffer(), original.framebuffer());
}

/// ROM yang membuat banyak field berisi nilai bukan-bawaan: EI + interrupt
/// timer, kanal noise, window, RAM cartridge MBC1 yang ditulis dan bank yang
/// diganti.
fn busy_rom() -> Box<[u8]> {
    let main = [
        0x3E, 0x80, 0xE0, 0x26, // APU menyala
        0x3E, 0xF3, 0xE0, 0x21, // noise: volume 15, turun, periode 3
        0x3E, 0x21, 0xE0, 0x22, // noise: geser 2, lebar 15 bit, pembagi 1
        0x3E, 0xC0, 0xE0, 0x23, // trigger + length
        0x3E, 0x05, 0xE0, 0x07, // TAC: timer menyala, 262 kHz
        0x3E, 0x04, 0xE0, 0xFF, // IE: timer
        0x3E, 0x0A, 0xEA, 0x00, 0x00, // RAM cartridge menyala
        0xAF, 0xE0, 0x4A, //       XOR A / LDH ($4A),A  WY = 0
        0x3E, 0x07, 0xE0, 0x4B, // WX = 7
        0x3E, 0xB1, 0xE0, 0x40, // LCDC: + window
        0xFB, //                   EI
        0x21, 0x00, 0xA0, //       LD HL,$A000
        0x34, //                   loop: INC (HL)    RAM cartridge
        0x7E, //                   LD A,(HL)
        0xE6, 0x03, //             AND 3
        0xEA, 0x00, 0x20, //       LD ($2000),A      ganti bank ROM
        0xF3, 0xFB, //             DI / EI           ei_pending sering menyala
        0x18, 0xF5, //             JR loop
    ];
    // Interrupt timer (0x50): naikkan byte di HRAM FF80.
    let handler = [0xF5, 0xF0, 0x80, 0x3C, 0xE0, 0x80, 0xF1, 0xD9];
    let mut rom = vec![0u8; 0x10000]; // 4 bank: MBC1 sungguhan
    rom[0x50..0x50 + handler.len()].copy_from_slice(&handler);
    rom[0x100..0x100 + main.len()].copy_from_slice(&main);
    rom[0x147] = 0x03; // MBC1 + RAM + baterai
    rom[0x148] = 0x01; // 64 KiB
    rom[0x149] = 0x02; // RAM 8 KiB
    rom.into_boxed_slice()
}

/// Bukti yang lebih kuat: setelah dimuat, mesin baru sama dengan aslinya
/// *field demi field* — termasuk field yang tidak ditulis ke state. Field
/// yang lupa disimpan langsung terlihat, tanpa menunggu efeknya.
///
/// State diambil setiap 1–11 instruksi, berganti-ganti — bukan hanya di
/// batas frame (tempat banyak field kebetulan bernilai bawaan) dan bukan
/// dengan jarak tetap: jarak 7 sama dengan panjang loop ROM, sehingga titik
/// simpan selalu jatuh di fase loop yang sama dan tidak pernah tepat
/// setelah `EI` (*aliasing*; begitulah `ei_pending` yang lupa disimpan
/// pernah lolos).
#[test]
fn every_field_survives_a_save_and_load() {
    let mut original = GameBoy::new(busy_rom()).expect("ROM valid");
    let fresh = GameBoy::new(busy_rom()).expect("ROM valid");
    for checkpoint in 0..3000 {
        for _ in 0..=checkpoint % 11 {
            original.step().expect("tidak ada opcode yang hilang");
        }
        let mut restored = fresh.clone();
        restored
            .load_state(&original.save_state())
            .expect("state valid");
        assert_eq!(
            restored.first_difference(&original),
            None,
            "titik simpan ke-{checkpoint}"
        );
    }
}

#[test]
fn a_state_starts_with_its_header() {
    let gb = GameBoy::new(rom()).expect("ROM valid");
    let state = gb.save_state();
    assert_eq!(&state[..4], b"MXMS");
    assert_eq!(u16::from_le_bytes([state[4], state[5]]), VERSION);
}

#[test]
fn bad_states_are_rejected_and_leave_the_machine_untouched() {
    let mut gb = GameBoy::new(rom()).expect("ROM valid");
    run(&mut gb, 5);
    let good = gb.save_state();
    let before = gb.save_state();

    assert_eq!(gb.load_state(b"bukan state"), Err(StateError::NotAState));

    let mut old_version = good.clone();
    old_version[4] = 0xFF;
    assert!(matches!(
        gb.load_state(&old_version),
        Err(StateError::Version(_))
    ));

    let mut other_rom = rom().into_vec();
    other_rom[0x7FFF] = 1;
    let other = GameBoy::new(other_rom.into_boxed_slice()).expect("ROM valid");
    assert_eq!(
        gb.load_state(&other.save_state()),
        Err(StateError::WrongRom)
    );

    // Terpotong di tengah: sebagian komponen sudah terbaca saat error.
    assert_eq!(
        gb.load_state(&good[..good.len() / 2]),
        Err(StateError::Corrupt)
    );

    let mut longer = good.clone();
    longer.push(0);
    assert_eq!(
        gb.load_state(&longer),
        Err(StateError::Corrupt),
        "sisa byte"
    );

    assert_eq!(gb.save_state(), before, "mesin tidak berubah setelah gagal");
}
