//! Tes CPU: satu kelompok per instruksi.
//!
//! Setiap tes menaruh program kecil di alamat 0x0100 (tempat CPU mulai),
//! menjalankan beberapa langkah, lalu memeriksa register, flag, memori, dan
//! jumlah siklus. Nilai yang diharapkan diambil dari tabel opcode publik
//! (Pan Docs), bukan dari keluaran emulator ini sendiri.

use super::{Cpu, CpuError, Flag};
use crate::bus::{Bus, FlatBus};

const START: u16 = 0x0100;

/// CPU dengan semua register nol (PC = 0x0100), dan bus berisi `program`.
fn setup(program: &[u8]) -> (Cpu, FlatBus) {
    let mut cpu = Cpu::new();
    cpu.regs = super::Registers::default();
    cpu.regs.pc = START;
    (cpu, FlatBus::with_program(START, program))
}

/// Menjalankan satu instruksi; tes gagal kalau opcode belum ditulis.
fn step(cpu: &mut Cpu, bus: &mut FlatBus) -> u8 {
    cpu.step(bus).expect("opcode harus sudah ditulis")
}

/// Flag dalam urutan Z N H C, supaya pesan gagal mudah dibaca.
fn flags(cpu: &Cpu) -> (bool, bool, bool, bool) {
    let r = &cpu.regs;
    (
        r.flag(Flag::Zero),
        r.flag(Flag::Subtract),
        r.flag(Flag::HalfCarry),
        r.flag(Flag::Carry),
    )
}

#[test]
fn starts_in_post_boot_state() {
    let cpu = Cpu::new();
    assert_eq!(cpu.regs.af(), 0x01B0);
    assert_eq!(cpu.regs.bc(), 0x0013);
    assert_eq!(cpu.regs.de(), 0x00D8);
    assert_eq!(cpu.regs.hl(), 0x014D);
    assert_eq!(cpu.regs.sp, 0xFFFE);
    assert_eq!(cpu.regs.pc, 0x0100);
}

#[test]
fn nop_only_moves_pc() {
    let (mut cpu, mut bus) = setup(&[0x00]);
    let before = cpu.regs;
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(cpu.regs.pc, START + 1);
    let mut expected = before;
    expected.pc = START + 1;
    assert_eq!(cpu.regs, expected);
}

#[test]
fn ld_rr_d16_is_little_endian() {
    // LD BC,0x1234 ; LD DE,0x5678 ; LD HL,0x9ABC ; LD SP,0xFFFE
    let (mut cpu, mut bus) = setup(&[
        0x01, 0x34, 0x12, 0x11, 0x78, 0x56, 0x21, 0xBC, 0x9A, 0x31, 0xFE, 0xFF,
    ]);
    for _ in 0..4 {
        assert_eq!(step(&mut cpu, &mut bus), 12);
    }
    assert_eq!(cpu.regs.bc(), 0x1234);
    assert_eq!(cpu.regs.de(), 0x5678);
    assert_eq!(cpu.regs.hl(), 0x9ABC);
    assert_eq!(cpu.regs.sp, 0xFFFE);
    assert_eq!(cpu.regs.pc, START + 12);
}

#[test]
fn ld_r_d8_fills_every_register() {
    // LD B,1 ; LD C,2 ; LD D,3 ; LD E,4 ; LD H,5 ; LD L,6 ; LD A,7
    let (mut cpu, mut bus) = setup(&[
        0x06, 1, 0x0E, 2, 0x16, 3, 0x1E, 4, 0x26, 5, 0x2E, 6, 0x3E, 7,
    ]);
    for _ in 0..7 {
        assert_eq!(step(&mut cpu, &mut bus), 8);
    }
    let r = cpu.regs;
    assert_eq!([r.b, r.c, r.d, r.e, r.h, r.l, r.a], [1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn ld_hl_d8_writes_memory() {
    // LD HL,0xC000 ; LD (HL),0x42
    let (mut cpu, mut bus) = setup(&[0x21, 0x00, 0xC0, 0x36, 0x42]);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(bus.read(0xC000), 0x42);
}

#[test]
fn ld_r_r_copies_between_registers() {
    // LD B,0x99 ; LD C,B ; LD A,C
    let (mut cpu, mut bus) = setup(&[0x06, 0x99, 0x48, 0x79]);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!((cpu.regs.b, cpu.regs.c, cpu.regs.a), (0x99, 0x99, 0x99));
}

#[test]
fn ld_through_hl_reads_and_writes_memory() {
    // LD HL,0xC000 ; LD A,0x5A ; LD (HL),A ; LD B,(HL)
    let (mut cpu, mut bus) = setup(&[0x21, 0x00, 0xC0, 0x3E, 0x5A, 0x77, 0x46]);
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(bus.read(0xC000), 0x5A);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.b, 0x5A);
}

#[test]
fn inc_sets_half_carry_and_keeps_carry() {
    let (mut cpu, mut bus) = setup(&[0x04]); // INC B
    cpu.regs.b = 0x0F;
    cpu.regs.set_flag(Flag::Carry, true);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(cpu.regs.b, 0x10);
    // Z N H C: H menyala (bit 3 → 4), C tetap seperti sebelumnya.
    assert_eq!(flags(&cpu), (false, false, true, true));
}

#[test]
fn inc_wraps_to_zero() {
    let (mut cpu, mut bus) = setup(&[0x3C]); // INC A
    cpu.regs.a = 0xFF;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x00);
    // INC tidak menyentuh C, walaupun hasilnya "meluap".
    assert_eq!(flags(&cpu), (true, false, true, false));
}

#[test]
fn inc_hl_changes_memory() {
    // LD HL,0xC000 ; INC (HL)
    let (mut cpu, mut bus) = setup(&[0x21, 0x00, 0xC0, 0x34]);
    bus.write(0xC000, 0x41);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(bus.read(0xC000), 0x42);
}

#[test]
fn dec_to_zero_sets_zero_and_subtract() {
    let (mut cpu, mut bus) = setup(&[0x0D]); // DEC C
    cpu.regs.c = 0x01;
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(cpu.regs.c, 0x00);
    assert_eq!(flags(&cpu), (true, true, false, false));
}

#[test]
fn dec_borrow_from_bit_4_sets_half_carry() {
    let (mut cpu, mut bus) = setup(&[0x05]); // DEC B
    cpu.regs.b = 0x10;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.b, 0x0F);
    assert_eq!(flags(&cpu), (false, true, true, false));
}

#[test]
fn dec_wraps_below_zero() {
    let (mut cpu, mut bus) = setup(&[0x3D]); // DEC A
    cpu.regs.a = 0x00;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0xFF);
    assert_eq!(flags(&cpu), (false, true, true, false));
}

#[test]
fn add_a_r_sets_all_flags() {
    // 0x3A + 0xC6 = 0x100 → A = 0x00: Z, H (0xA + 0x6 = 0x10), C.
    let (mut cpu, mut bus) = setup(&[0x80]); // ADD A,B
    cpu.regs.a = 0x3A;
    cpu.regs.b = 0xC6;
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, false, true, true));
}

#[test]
fn add_a_without_carries_clears_flags() {
    let (mut cpu, mut bus) = setup(&[0x81]); // ADD A,C
    cpu.regs.a = 0x12;
    cpu.regs.c = 0x34;
    cpu.regs.set_f(0xF0);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x46);
    assert_eq!(flags(&cpu), (false, false, false, false));
}

#[test]
fn add_a_half_carry_boundary() {
    // Kasus batas: 0x8 + 0x7 = 0xF masih muat di 4 bit → H TIDAK menyala.
    // 0x8 + 0x8 = 0x10 tepat melewati batas → H menyala.
    let (mut cpu, mut bus) = setup(&[0x80, 0x80]); // ADD A,B ; ADD A,B
    cpu.regs.a = 0x08;
    cpu.regs.b = 0x07;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x0F);
    assert!(!cpu.regs.flag(Flag::HalfCarry));

    cpu.regs.a = 0x08;
    cpu.regs.b = 0x08;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x10);
    assert!(cpu.regs.flag(Flag::HalfCarry));
}

#[test]
fn add_a_carry_boundary() {
    // 0xFF + 0x00 tidak meluap; 0xFF + 0x01 meluap.
    let (mut cpu, mut bus) = setup(&[0x80, 0x80]);
    cpu.regs.a = 0xFF;
    step(&mut cpu, &mut bus);
    assert!(!cpu.regs.flag(Flag::Carry));

    cpu.regs.b = 0x01;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x00);
    assert!(cpu.regs.flag(Flag::Carry));
}

#[test]
fn add_a_d8_and_add_a_hl() {
    // LD HL,0xC000 ; ADD A,0x05 ; ADD A,(HL)
    let (mut cpu, mut bus) = setup(&[0x21, 0x00, 0xC0, 0xC6, 0x05, 0x86]);
    bus.write(0xC000, 0x10);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.a, 0x05);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.a, 0x15);
}

#[test]
fn jp_jumps_and_loops() {
    // 0x0100: INC A ; JP 0x0100  → sebuah loop tanpa akhir.
    let (mut cpu, mut bus) = setup(&[0x3C, 0xC3, 0x00, 0x01]);
    for round in 1..=3 {
        step(&mut cpu, &mut bus);
        assert_eq!(step(&mut cpu, &mut bus), 16);
        assert_eq!(cpu.regs.pc, START);
        assert_eq!(cpu.regs.a, round);
    }
}

#[test]
fn unimplemented_opcode_leaves_cpu_unchanged() {
    let (mut cpu, mut bus) = setup(&[0xD3]); // 0xD3 tidak ada di SM83
    let before = cpu.clone();
    assert_eq!(
        cpu.step(&mut bus),
        Err(CpuError::Unimplemented {
            opcode: 0xD3,
            address: START
        })
    );
    assert_eq!(cpu, before);
}

#[test]
fn halt_is_not_treated_as_ld() {
    let (mut cpu, mut bus) = setup(&[0x76]);
    assert!(matches!(
        cpu.step(&mut bus),
        Err(CpuError::Unimplemented { opcode: 0x76, .. })
    ));
}
