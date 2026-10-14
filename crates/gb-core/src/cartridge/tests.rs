use alloc::boxed::Box;
use alloc::vec;

use super::{Cartridge, CartridgeError, Mbc, ROM_BANK};

/// ROM `banks` × 16 KiB. Byte pertama setiap bank berisi nomor bank-nya
/// (bit 0–7) dan byte kedua bit 8, supaya tes bisa melihat bank mana yang
/// sedang terlihat.
fn rom(kind: u8, banks: usize, ram_code: u8) -> Box<[u8]> {
    let mut rom = vec![0u8; banks * ROM_BANK];
    for bank in 0..banks {
        let [low, high, ..] = bank.to_le_bytes();
        rom[bank * ROM_BANK] = low;
        rom[bank * ROM_BANK + 1] = high;
    }
    rom[0x134..0x139].copy_from_slice(b"TESTS");
    rom[0x147] = kind;
    rom[0x149] = ram_code;
    let checksum = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |sum, &byte| sum.wrapping_sub(byte).wrapping_sub(1));
    rom[0x14D] = checksum;
    rom.into_boxed_slice()
}

fn cart(kind: u8, banks: usize, ram_code: u8) -> Cartridge {
    Cartridge::new(rom(kind, banks, ram_code)).expect("ROM tes valid")
}

/// Nomor bank yang terlihat di 4000–7FFF.
fn high_bank(cart: &Cartridge) -> usize {
    usize::from(cart.read_rom(0x4000)) | usize::from(cart.read_rom(0x4001)) << 8
}

/// Nomor bank yang terlihat di 0000–3FFF.
fn low_bank(cart: &Cartridge) -> usize {
    usize::from(cart.read_rom(0x0000)) | usize::from(cart.read_rom(0x0001)) << 8
}

#[test]
fn header_is_parsed() {
    let cart = cart(0x03, 4, 3);
    let header = cart.header();
    assert_eq!(header.title, "TESTS");
    assert_eq!(header.kind, 0x03);
    assert_eq!(header.ram_size, 0x8000);
    assert!(header.battery);
    assert!(header.checksum_ok);
    assert_eq!(cart.ram().len(), 0x8000);
}

#[test]
fn wrong_header_checksum_is_reported_but_still_runs() {
    let mut bytes = rom(0x00, 2, 0);
    bytes[0x14D] ^= 1;
    let cart = Cartridge::new(bytes).expect("tetap boleh jalan");
    assert!(!cart.header().checksum_ok);
}

#[test]
fn rejects_tiny_files_and_unknown_types() {
    assert_eq!(
        Cartridge::new(vec![0; 16].into_boxed_slice()).err(),
        Some(CartridgeError::TooSmall(16))
    );
    assert_eq!(
        Cartridge::new(rom(0xFC, 2, 0)).err(),
        Some(CartridgeError::Unsupported(0xFC))
    );
}

#[test]
fn rom_only_shows_bank_1_and_ignores_writes() {
    let mut cart = cart(0x00, 2, 0);
    cart.write_rom(0x2000, 0x05);
    assert_eq!((low_bank(&cart), high_bank(&cart)), (0, 1));
    assert_eq!(cart.read_ram(0xA000), 0xFF, "tanpa RAM");
}

#[test]
fn mbc1_switches_rom_banks_and_zero_means_one() {
    let mut cart = cart(0x01, 8, 0);
    cart.write_rom(0x2000, 5);
    assert_eq!(high_bank(&cart), 5);
    cart.write_rom(0x2000, 0);
    assert_eq!(high_bank(&cart), 1, "bank 0 tidak bisa dipilih di 4000");
    cart.write_rom(0x2000, 0xE3); // hanya 5 bit bawah: 3
    assert_eq!(high_bank(&cart), 3);
    cart.write_rom(0x2000, 9); // ROM hanya 8 bank: di-mask jadi 1
    assert_eq!(high_bank(&cart), 1);
}

#[test]
fn mbc1_bank2_adds_upper_bits_and_bank_0x20_is_unreachable() {
    let mut cart = cart(0x01, 128, 0);
    cart.write_rom(0x4000, 1);
    cart.write_rom(0x2000, 0);
    // BANK1 = 0 dibaca 1 sebelum digabung: 0x21, bukan 0x20.
    assert_eq!(high_bank(&cart), 0x21);
    assert_eq!(low_bank(&cart), 0, "mode 0: 0000–3FFF tetap bank 0");
    cart.write_rom(0x6000, 1);
    assert_eq!(low_bank(&cart), 0x20, "mode 1: BANK2 juga di 0000–3FFF");
}

#[test]
fn mbc1_ram_needs_enable_and_bank2_selects_ram_bank_in_mode_1() {
    let mut cart = cart(0x03, 4, 3); // RAM 32 KiB = 4 bank
    cart.write_ram(0xA000, 0x11);
    assert_eq!(cart.read_ram(0xA000), 0xFF, "RAM mati: tulisan diabaikan");
    cart.write_rom(0x0000, 0x0A);
    cart.write_ram(0xA000, 0x11);
    cart.write_rom(0x6000, 1);
    cart.write_rom(0x4000, 2);
    cart.write_ram(0xA000, 0x22);
    assert_eq!(cart.read_ram(0xA000), 0x22);
    cart.write_rom(0x6000, 0); // mode 0: selalu bank RAM 0
    assert_eq!(cart.read_ram(0xA000), 0x11);
    assert_eq!(&cart.ram()[0x4000..0x4001], &[0x22]);
    cart.write_rom(0x0000, 0x00);
    assert_eq!(cart.read_ram(0xA000), 0xFF);
}

#[test]
fn ram_enable_only_looks_at_the_low_4_bits() {
    let mut cart = cart(0x03, 4, 2);
    cart.write_rom(0x1FFF, 0x1A); // 4 bit bawah = 0xA: menyala
    cart.write_ram(0xA000, 0x55);
    assert_eq!(cart.read_ram(0xA000), 0x55);
    cart.write_rom(0x0000, 0xA0); // 4 bit bawah = 0x0: mati
    assert_eq!(cart.read_ram(0xA000), 0xFF);
}

#[test]
fn mbc2_uses_address_bit_8_and_has_4_bit_ram() {
    let mut cart = cart(0x06, 16, 0);
    cart.write_rom(0x2100, 7); // bit 8 = 1: bank ROM
    assert_eq!(high_bank(&cart), 7);
    cart.write_rom(0x2000, 0x0A); // bit 8 = 0: RAM, bukan bank
    assert_eq!(high_bank(&cart), 7);
    cart.write_ram(0xA000, 0x5C);
    assert_eq!(
        cart.read_ram(0xA000),
        0xFC,
        "hanya 4 bit; bit atas terbaca 1"
    );
    assert_eq!(cart.read_ram(0xA200), 0xFC, "512 sel dicerminkan");
    assert_eq!(cart.ram().len(), 512);
}

#[test]
fn mbc3_has_7_bit_rom_banks_ram_banks_and_frozen_clock() {
    let mut cart = cart(0x10, 128, 3);
    cart.write_rom(0x2000, 0x7F);
    assert_eq!(high_bank(&cart), 0x7F);
    cart.write_rom(0x0000, 0x0A);
    cart.write_rom(0x4000, 3);
    cart.write_ram(0xA123, 0x33);
    assert_eq!(cart.ram()[3 * 0x2000 + 0x123], 0x33);
    cart.write_rom(0x4000, 0x08); // register detik
    cart.write_ram(0xA000, 42);
    assert_eq!(cart.read_ram(0xA000), 42);
    cart.write_rom(0x4000, 3);
    assert_eq!(cart.read_ram(0xA123), 0x33, "jam tidak menimpa RAM");
    cart.write_rom(0x4000, 0x05); // bukan bank RAM, bukan jam
    assert_eq!(cart.read_ram(0xA000), 0xFF);
}

#[test]
fn mbc5_has_9_bit_rom_banks_and_bank_zero_is_allowed() {
    let mut cart = cart(0x1B, 512, 4);
    cart.write_rom(0x2000, 0);
    assert_eq!(high_bank(&cart), 0, "MBC5 boleh memilih bank 0");
    cart.write_rom(0x2000, 0x34);
    cart.write_rom(0x3000, 1);
    assert_eq!(high_bank(&cart), 0x134);
    cart.write_rom(0x2000, 0x01);
    assert_eq!(high_bank(&cart), 0x101, "bit 8 tetap");
    cart.write_rom(0x0000, 0x0A);
    cart.write_rom(0x4000, 0x0F);
    cart.write_ram(0xBFFF, 0x77);
    assert_eq!(cart.ram()[15 * 0x2000 + 0x1FFF], 0x77);
}

#[test]
fn ram_survives_a_save_and_load() {
    let mut cart = cart(0x03, 4, 2);
    cart.write_rom(0x0000, 0x0A);
    cart.write_ram(0xA010, 0x99);
    let saved = cart.ram().to_vec();
    let mut fresh = Cartridge::new(rom(0x03, 4, 2)).expect("ROM tes valid");
    fresh.load_ram(&saved);
    fresh.write_rom(0x0000, 0x0A);
    assert_eq!(fresh.read_ram(0xA010), 0x99);
    assert!(matches!(fresh.mbc(), Mbc::Mbc1 { .. }));
}

#[test]
fn ram_writes_raise_the_dirty_flag_once() {
    let mut cart = cart(0x03, 4, 2);
    assert!(!cart.take_ram_dirty(), "baru dipasang: belum berubah");
    cart.write_ram(0xA000, 0x01);
    assert!(!cart.take_ram_dirty(), "RAM mati: tidak ada yang berubah");
    cart.write_rom(0x0000, 0x0A);
    cart.write_ram(0xA000, 0x01);
    assert!(cart.take_ram_dirty());
    assert!(!cart.take_ram_dirty(), "tanda dihapus setelah dibaca");
    cart.write_ram(0xA000, 0x01);
    assert!(
        !cart.take_ram_dirty(),
        "nilai sama: tidak perlu disimpan ulang"
    );
}
