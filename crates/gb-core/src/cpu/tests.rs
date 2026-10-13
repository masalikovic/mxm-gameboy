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

// ---------------------------------------------------------------------------
// ALU: ADC, SUB, SBC, AND, XOR, OR, CP
// ---------------------------------------------------------------------------

/// Menjalankan satu instruksi ALU A,B dengan A, B, dan flag C awal tertentu.
fn alu_ab(opcode: u8, a: u8, b: u8, carry_in: bool) -> Cpu {
    let (mut cpu, mut bus) = setup(&[opcode]);
    cpu.regs.a = a;
    cpu.regs.b = b;
    cpu.regs.set_flag(Flag::Carry, carry_in);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    cpu
}

#[test]
fn adc_adds_the_carry_flag() {
    // 0x0E + 0x01 + C(1) = 0x10 → H menyala karena carry ikut dihitung.
    let cpu = alu_ab(0x88, 0x0E, 0x01, true); // ADC A,B
    assert_eq!(cpu.regs.a, 0x10);
    assert_eq!(flags(&cpu), (false, false, true, false));

    // Tanpa carry: 0x0E + 0x01 = 0x0F → H tidak menyala (kasus batas).
    let cpu = alu_ab(0x88, 0x0E, 0x01, false);
    assert_eq!(cpu.regs.a, 0x0F);
    assert_eq!(flags(&cpu), (false, false, false, false));

    // 0xFF + 0x00 + C(1) = 0x100 → A = 0, Z, H, C.
    let cpu = alu_ab(0x88, 0xFF, 0x00, true);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, false, true, true));
}

#[test]
fn sub_sets_borrow_flags() {
    // 0x3E - 0x0F: pinjam dari bit 4 (H), tidak pinjam dari bit 8.
    let cpu = alu_ab(0x90, 0x3E, 0x0F, false); // SUB B
    assert_eq!(cpu.regs.a, 0x2F);
    assert_eq!(flags(&cpu), (false, true, true, false));

    // 0x10 - 0x20 → negatif: C menyala, hasil membungkus ke 0xF0.
    let cpu = alu_ab(0x90, 0x10, 0x20, false);
    assert_eq!(cpu.regs.a, 0xF0);
    assert_eq!(flags(&cpu), (false, true, false, true));

    // Sama dengan sama → nol.
    let cpu = alu_ab(0x90, 0x42, 0x42, true);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, true, false, false));
}

#[test]
fn sbc_subtracts_the_carry_flag() {
    // 0x10 - 0x0F - C(1) = 0x00. Kasus batas: tepat nol, tanpa pinjam bit 8.
    let cpu = alu_ab(0x98, 0x10, 0x0F, true); // SBC A,B
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, true, true, false));

    // 0x00 - 0x00 - C(1) = 0xFF: pinjam di bit 4 dan bit 8.
    let cpu = alu_ab(0x98, 0x00, 0x00, true);
    assert_eq!(cpu.regs.a, 0xFF);
    assert_eq!(flags(&cpu), (false, true, true, true));

    // Tanpa carry, SBC = SUB.
    let cpu = alu_ab(0x98, 0x05, 0x03, false);
    assert_eq!(cpu.regs.a, 0x02);
    assert_eq!(flags(&cpu), (false, true, false, false));
}

#[test]
fn and_always_sets_half_carry() {
    let cpu = alu_ab(0xA0, 0b1100_1010, 0b1010_0110, true); // AND B
    assert_eq!(cpu.regs.a, 0b1000_0010);
    assert_eq!(flags(&cpu), (false, false, true, false));

    let cpu = alu_ab(0xA0, 0xF0, 0x0F, false);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, false, true, false));
}

#[test]
fn xor_and_or_clear_other_flags() {
    let cpu = alu_ab(0xA8, 0b1100_1010, 0b1010_0110, true); // XOR B
    assert_eq!(cpu.regs.a, 0b0110_1100);
    assert_eq!(flags(&cpu), (false, false, false, false));

    let cpu = alu_ab(0xB0, 0b1100_0000, 0b0000_0011, true); // OR B
    assert_eq!(cpu.regs.a, 0b1100_0011);
    assert_eq!(flags(&cpu), (false, false, false, false));

    let cpu = alu_ab(0xB0, 0x00, 0x00, false);
    assert_eq!(flags(&cpu), (true, false, false, false));
}

#[test]
fn xor_a_is_the_classic_way_to_zero_a() {
    // XOR A (0xAF) adalah cara 1 byte paling umum untuk A = 0 di game asli.
    let (mut cpu, mut bus) = setup(&[0xAF]);
    cpu.regs.a = 0x5C;
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, false, false, false));
}

#[test]
fn cp_compares_without_changing_a() {
    // A == B → Z. A tetap.
    let cpu = alu_ab(0xB8, 0x42, 0x42, false); // CP B
    assert_eq!(cpu.regs.a, 0x42);
    assert_eq!(flags(&cpu), (true, true, false, false));

    // A < B → C.
    let cpu = alu_ab(0xB8, 0x10, 0x11, false);
    assert_eq!(cpu.regs.a, 0x10);
    assert_eq!(flags(&cpu), (false, true, true, true));

    // A > B → bukan Z, bukan C.
    let cpu = alu_ab(0xB8, 0x11, 0x10, true);
    assert_eq!(flags(&cpu), (false, true, false, false));
}

#[test]
fn alu_immediate_variants_use_next_byte() {
    // LD A,0x0F ; AND 0x3C ; OR 0x80 ; XOR 0xFF ; SUB 0x01 ; CP 0x72
    let (mut cpu, mut bus) = setup(&[
        0x3E, 0x0F, 0xE6, 0x3C, 0xF6, 0x80, 0xEE, 0xFF, 0xD6, 0x01, 0xFE, 0x72,
    ]);
    step(&mut cpu, &mut bus);
    for expected in [0x0C, 0x8C, 0x73, 0x72] {
        assert_eq!(step(&mut cpu, &mut bus), 8);
        assert_eq!(cpu.regs.a, expected);
    }
    assert_eq!(step(&mut cpu, &mut bus), 8); // CP 0x72
    assert_eq!(cpu.regs.a, 0x72);
    assert!(cpu.regs.flag(Flag::Zero));
}

#[test]
fn adc_and_sbc_immediate_use_carry() {
    // ADC A,0x01 lalu SBC A,0x01, dengan C menyala di awal.
    let (mut cpu, mut bus) = setup(&[0xCE, 0x01, 0xDE, 0x01]);
    cpu.regs.a = 0x10;
    cpu.regs.set_flag(Flag::Carry, true);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x12); // 0x10 + 0x01 + 1
    cpu.regs.set_flag(Flag::Carry, true);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x10); // 0x12 - 0x01 - 1
}

#[test]
fn alu_with_hl_reads_memory() {
    // LD HL,0xC000 ; SUB (HL)
    let (mut cpu, mut bus) = setup(&[0x21, 0x00, 0xC0, 0x96]);
    bus.write(0xC000, 0x05);
    cpu.regs.a = 0x08;
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.a, 0x03);
}

// ---------------------------------------------------------------------------
// Lompatan: JR, JR cc, JP cc, JP HL
// ---------------------------------------------------------------------------

#[test]
fn jr_jumps_forward_and_backward() {
    // 0x0100: JR +2  → lewati 2 byte, mendarat di 0x0104.
    let (mut cpu, mut bus) = setup(&[0x18, 0x02, 0x00, 0x00, 0x18, 0xFA]);
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(cpu.regs.pc, 0x0104);
    // 0x0104: JR -6 (0xFA) → 0x0106 - 6 = 0x0100.
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(cpu.regs.pc, 0x0100);
}

#[test]
fn jr_offset_extremes() {
    // +127 (0x7F) dan -128 (0x80) adalah batas offset 8-bit bertanda.
    let (mut cpu, mut bus) = setup(&[0x18, 0x7F]);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.pc, 0x0102 + 127);

    let (mut cpu, mut bus) = setup(&[0x18, 0x80]);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.pc, 0x0102 - 128);
}

#[test]
fn jr_cc_taken_and_not_taken() {
    // Setiap baris: (opcode, Z, C, lompat?)
    let cases = [
        (0x20, false, false, true), // JR NZ
        (0x20, true, false, false),
        (0x28, true, false, true), // JR Z
        (0x28, false, false, false),
        (0x30, false, false, true), // JR NC
        (0x30, false, true, false),
        (0x38, false, true, true), // JR C
        (0x38, false, false, false),
    ];
    for (opcode, z, c, taken) in cases {
        let (mut cpu, mut bus) = setup(&[opcode, 0x10]);
        cpu.regs.set_flag(Flag::Zero, z);
        cpu.regs.set_flag(Flag::Carry, c);
        let cycles = step(&mut cpu, &mut bus);
        let expected_pc = if taken { 0x0102 + 0x10 } else { 0x0102 };
        assert_eq!(cpu.regs.pc, expected_pc, "opcode {opcode:#04X} Z={z} C={c}");
        assert_eq!(cycles, if taken { 12 } else { 8 }, "opcode {opcode:#04X}");
    }
}

#[test]
fn jp_cc_taken_and_not_taken() {
    let cases = [
        (0xC2, false, true),
        (0xCA, false, false),
        (0xD2, true, false),
        (0xDA, true, true),
    ];
    for (opcode, carry, taken) in cases {
        let (mut cpu, mut bus) = setup(&[opcode, 0x34, 0x12]);
        cpu.regs.set_flag(Flag::Zero, false);
        cpu.regs.set_flag(Flag::Carry, carry);
        let cycles = step(&mut cpu, &mut bus);
        assert_eq!(
            cpu.regs.pc,
            if taken { 0x1234 } else { 0x0103 },
            "opcode {opcode:#04X}"
        );
        assert_eq!(cycles, if taken { 16 } else { 12 });
    }
}

#[test]
fn jp_hl_uses_the_register_not_memory() {
    let (mut cpu, mut bus) = setup(&[0xE9]);
    cpu.regs.set_hl(0x4000);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert_eq!(cpu.regs.pc, 0x4000);
}

// ---------------------------------------------------------------------------
// Stack: PUSH, POP, CALL, RET
// ---------------------------------------------------------------------------

#[test]
fn push_grows_stack_downward() {
    let (mut cpu, mut bus) = setup(&[0xC5]); // PUSH BC
    cpu.regs.sp = 0xFFFE;
    cpu.regs.set_bc(0x1234);
    assert_eq!(step(&mut cpu, &mut bus), 16);
    assert_eq!(cpu.regs.sp, 0xFFFC);
    // Byte tinggi di alamat lebih tinggi, byte rendah di SP.
    assert_eq!(bus.read(0xFFFD), 0x12);
    assert_eq!(bus.read(0xFFFC), 0x34);
}

#[test]
fn push_then_pop_moves_value_between_pairs() {
    // PUSH BC ; POP DE  → DE = BC, SP kembali seperti semula.
    let (mut cpu, mut bus) = setup(&[0xC5, 0xD1]);
    cpu.regs.sp = 0xFFFE;
    cpu.regs.set_bc(0xBEEF);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(cpu.regs.de(), 0xBEEF);
    assert_eq!(cpu.regs.sp, 0xFFFE);
}

#[test]
fn push_pop_all_pairs() {
    // PUSH BC ; PUSH DE ; PUSH HL ; POP BC ; POP DE ; POP HL → BC↔HL tertukar.
    let (mut cpu, mut bus) = setup(&[0xC5, 0xD5, 0xE5, 0xC1, 0xD1, 0xE1]);
    cpu.regs.sp = 0xD000;
    cpu.regs.set_bc(0x1111);
    cpu.regs.set_de(0x2222);
    cpu.regs.set_hl(0x3333);
    for _ in 0..6 {
        step(&mut cpu, &mut bus);
    }
    assert_eq!(
        (cpu.regs.bc(), cpu.regs.de(), cpu.regs.hl()),
        (0x3333, 0x2222, 0x1111)
    );
}

#[test]
fn pop_af_drops_low_nibble_of_f() {
    // Taruh 0x12FF di stack, lalu POP AF: F hanya boleh 0xF0.
    let (mut cpu, mut bus) = setup(&[0xF1]);
    cpu.regs.sp = 0xC000;
    bus.write(0xC000, 0xFF); // byte rendah → F
    bus.write(0xC001, 0x12); // byte tinggi → A
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x12);
    assert_eq!(cpu.regs.f(), 0xF0);
}

#[test]
fn call_and_ret_return_to_the_next_instruction() {
    // 0x0100: CALL 0x0200 ; INC B
    // 0x0200: INC A ; RET
    let mut program = [0u8; 0x104];
    program[..4].copy_from_slice(&[0xCD, 0x00, 0x02, 0x04]);
    program[0x100..0x102].copy_from_slice(&[0x3C, 0xC9]);
    let (mut cpu, mut bus) = setup(&program);
    cpu.regs.sp = 0xFFFE;

    assert_eq!(step(&mut cpu, &mut bus), 24); // CALL
    assert_eq!(cpu.regs.pc, 0x0200);
    assert_eq!(cpu.regs.sp, 0xFFFC);
    // Alamat kembali = instruksi setelah CALL (0x0103).
    assert_eq!((bus.read(0xFFFD), bus.read(0xFFFC)), (0x01, 0x03));

    step(&mut cpu, &mut bus); // INC A
    assert_eq!(step(&mut cpu, &mut bus), 16); // RET
    assert_eq!(cpu.regs.pc, 0x0103);
    assert_eq!(cpu.regs.sp, 0xFFFE);

    step(&mut cpu, &mut bus); // INC B
    assert_eq!((cpu.regs.a, cpu.regs.b), (1, 1));
}

#[test]
fn call_cc_and_ret_cc_cycles() {
    // CALL Z tidak diambil (Z mati): 12 siklus, stack tidak disentuh.
    let (mut cpu, mut bus) = setup(&[0xCC, 0x00, 0x02]);
    cpu.regs.sp = 0xFFFE;
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!((cpu.regs.pc, cpu.regs.sp), (0x0103, 0xFFFE));

    // CALL NZ diambil (Z mati): 24 siklus.
    let (mut cpu, mut bus) = setup(&[0xC4, 0x00, 0x02]);
    cpu.regs.sp = 0xFFFE;
    assert_eq!(step(&mut cpu, &mut bus), 24);
    assert_eq!(cpu.regs.pc, 0x0200);

    // RET C tidak diambil: 8 siklus. RET NC diambil: 20 siklus.
    let (mut cpu, mut bus) = setup(&[0xD8, 0xD0]);
    cpu.regs.sp = 0xC000;
    bus.write(0xC000, 0x00);
    bus.write(0xC001, 0x40);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.pc, 0x0101);
    assert_eq!(step(&mut cpu, &mut bus), 20);
    assert_eq!((cpu.regs.pc, cpu.regs.sp), (0x4000, 0xC002));
}

#[test]
fn countdown_loop_runs_exactly_n_times() {
    // Program kecil seperti di game asli:
    //   LD B,3 ; loop: INC A ; DEC B ; JR NZ,loop
    let (mut cpu, mut bus) = setup(&[0x06, 0x03, 0x3C, 0x05, 0x20, 0xFC]);
    let mut total = u32::from(step(&mut cpu, &mut bus));
    while cpu.regs.pc != 0x0106 {
        total += u32::from(step(&mut cpu, &mut bus));
    }
    assert_eq!(cpu.regs.a, 3);
    assert_eq!(cpu.regs.b, 0);
    // 8 + 3×(4+4) + 2×12 (lompat) + 1×8 (tidak lompat) = 64 siklus.
    assert_eq!(total, 64);
}

// ---------------------------------------------------------------------------
// Memori lewat pasangan register, LDH, alamat 16-bit
// ---------------------------------------------------------------------------

#[test]
fn ld_through_bc_and_de() {
    // LD (BC),A ; LD A,(DE)
    let (mut cpu, mut bus) = setup(&[0x02, 0x1A]);
    cpu.regs.a = 0x77;
    cpu.regs.set_bc(0xC000);
    cpu.regs.set_de(0xC100);
    bus.write(0xC100, 0x33);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(bus.read(0xC000), 0x77);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.a, 0x33);
}

#[test]
fn ld_hl_plus_and_minus_move_hl_after_use() {
    // LD (HL+),A ; LD (HL+),A ; LD A,(HL-)
    let (mut cpu, mut bus) = setup(&[0x22, 0x22, 0x3A]);
    cpu.regs.set_hl(0xC000);
    cpu.regs.a = 0xAA;
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert_eq!((bus.read(0xC000), bus.read(0xC001)), (0xAA, 0xAA));
    assert_eq!(cpu.regs.hl(), 0xC002);
    bus.write(0xC002, 0x5B);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.a, 0x5B);
    assert_eq!(cpu.regs.hl(), 0xC001);
}

#[test]
fn ld_hl_minus_wraps_at_zero() {
    let (mut cpu, mut bus) = setup(&[0x32]); // LD (HL-),A
    cpu.regs.set_hl(0x0000);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.hl(), 0xFFFF);
}

#[test]
fn ldh_uses_page_ff() {
    // LDH (0x80),A ; LDH A,(0x81)
    let (mut cpu, mut bus) = setup(&[0xE0, 0x80, 0xF0, 0x81]);
    cpu.regs.a = 0x12;
    bus.write(0xFF81, 0x34);
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(bus.read(0xFF80), 0x12);
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(cpu.regs.a, 0x34);
}

#[test]
fn ld_c_indirect_uses_page_ff() {
    // LD (C),A ; LD A,(C)
    let (mut cpu, mut bus) = setup(&[0xE2, 0xF2]);
    cpu.regs.c = 0x90;
    cpu.regs.a = 0x99;
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(bus.read(0xFF90), 0x99);
    bus.write(0xFF90, 0x11);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.a, 0x11);
}

#[test]
fn ld_absolute_address() {
    // LD (0xC123),A ; LD A,(0xC200)
    let (mut cpu, mut bus) = setup(&[0xEA, 0x23, 0xC1, 0xFA, 0x00, 0xC2]);
    cpu.regs.a = 0x4D;
    bus.write(0xC200, 0x6E);
    assert_eq!(step(&mut cpu, &mut bus), 16);
    assert_eq!(bus.read(0xC123), 0x4D);
    assert_eq!(step(&mut cpu, &mut bus), 16);
    assert_eq!(cpu.regs.a, 0x6E);
}

#[test]
fn ld_a16_sp_stores_little_endian() {
    let (mut cpu, mut bus) = setup(&[0x08, 0x00, 0xC0]); // LD (0xC000),SP
    cpu.regs.sp = 0xBEEF;
    assert_eq!(step(&mut cpu, &mut bus), 20);
    assert_eq!((bus.read(0xC000), bus.read(0xC001)), (0xEF, 0xBE));
}

// ---------------------------------------------------------------------------
// Aritmetika 16-bit
// ---------------------------------------------------------------------------

#[test]
fn inc_dec_rr_wrap_and_leave_flags_alone() {
    // INC BC ; DEC DE ; INC SP
    let (mut cpu, mut bus) = setup(&[0x03, 0x1B, 0x33]);
    cpu.regs.set_bc(0xFFFF);
    cpu.regs.set_de(0x0000);
    cpu.regs.sp = 0x1234;
    cpu.regs.set_f(0xA0);
    for _ in 0..3 {
        assert_eq!(step(&mut cpu, &mut bus), 8);
    }
    assert_eq!(cpu.regs.bc(), 0x0000);
    assert_eq!(cpu.regs.de(), 0xFFFF);
    assert_eq!(cpu.regs.sp, 0x1235);
    // Tidak ada flag yang berubah, walaupun hasilnya nol atau meluap.
    assert_eq!(cpu.regs.f(), 0xA0);
}

#[test]
fn add_hl_half_carry_is_from_bit_11() {
    // 0x0FFF + 0x0001 = 0x1000: H menyala (bit 11 → 12), C tidak.
    let (mut cpu, mut bus) = setup(&[0x09, 0x09]); // ADD HL,BC ×2
    cpu.regs.set_hl(0x0FFF);
    cpu.regs.set_bc(0x0001);
    cpu.regs.set_flag(Flag::Zero, true);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.hl(), 0x1000);
    // Z tidak disentuh ADD HL; tetap menyala dari sebelumnya.
    assert_eq!(flags(&cpu), (true, false, true, false));

    // Kasus batas: 0x0FFE + 0x0001 = 0x0FFF, H tidak menyala.
    cpu.regs.set_hl(0x0FFE);
    step(&mut cpu, &mut bus);
    assert!(!cpu.regs.flag(Flag::HalfCarry));
}

#[test]
fn add_hl_half_carry_is_not_from_bit_7() {
    // Angka yang memisahkan bit 7 dan bit 11 (tes di atas tidak bisa):
    // 0x00FF + 0x0001 → carry di bit 7 saja → H MATI.
    // 0x0F00 + 0x0100 → carry di bit 11 saja → H MENYALA.
    let (mut cpu, mut bus) = setup(&[0x09, 0x09]); // ADD HL,BC ×2
    cpu.regs.set_hl(0x00FF);
    cpu.regs.set_bc(0x0001);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.hl(), 0x0100);
    assert!(!cpu.regs.flag(Flag::HalfCarry));

    cpu.regs.set_hl(0x0F00);
    cpu.regs.set_bc(0x0100);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.hl(), 0x1000);
    assert!(cpu.regs.flag(Flag::HalfCarry));
}

#[test]
fn add_hl_carry_from_bit_15() {
    let (mut cpu, mut bus) = setup(&[0x29]); // ADD HL,HL
    cpu.regs.set_hl(0x8000);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.hl(), 0x0000);
    // Hasil nol, tetapi Z tetap tidak disentuh (awalnya mati).
    assert_eq!(flags(&cpu), (false, false, false, true));
}

#[test]
fn add_sp_e8_positive_and_negative() {
    // SP = 0xD001, offset -1 (0xFF) → 0xD000.
    // Flag dari byte rendah tanpa tanda: 0x01 + 0xFF = 0x100 → H dan C menyala.
    let (mut cpu, mut bus) = setup(&[0xE8, 0xFF]);
    cpu.regs.sp = 0xD001;
    cpu.regs.set_flag(Flag::Zero, true);
    assert_eq!(step(&mut cpu, &mut bus), 16);
    assert_eq!(cpu.regs.sp, 0xD000);
    assert_eq!(flags(&cpu), (false, false, true, true));

    // SP = 0xD000, offset -1 → 0xCFFF. 0x00 + 0xFF = 0xFF → tanpa H dan C.
    let (mut cpu, mut bus) = setup(&[0xE8, 0xFF]);
    cpu.regs.sp = 0xD000;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.sp, 0xCFFF);
    assert_eq!(flags(&cpu), (false, false, false, false));

    // SP = 0x000F, offset +1 → H (0xF + 0x1); C tidak (0x0F + 0x01 < 0x100).
    let (mut cpu, mut bus) = setup(&[0xE8, 0x01]);
    cpu.regs.sp = 0x000F;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.sp, 0x0010);
    assert_eq!(flags(&cpu), (false, false, true, false));
}

#[test]
fn ld_hl_sp_e8_and_ld_sp_hl() {
    // LD HL,SP-2 ; LD SP,HL
    let (mut cpu, mut bus) = setup(&[0xF8, 0xFE, 0xF9]);
    cpu.regs.sp = 0xFFFE;
    assert_eq!(step(&mut cpu, &mut bus), 12);
    assert_eq!(cpu.regs.hl(), 0xFFFC);
    assert_eq!(cpu.regs.sp, 0xFFFE); // SP sendiri tidak berubah
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.sp, 0xFFFC);
}

// ---------------------------------------------------------------------------
// Rotasi A, DAA, CPL, SCF, CCF
// ---------------------------------------------------------------------------

/// Menjalankan satu operasi A dengan nilai A dan flag C awal tertentu.
fn a_op(opcode: u8, a: u8, carry_in: bool) -> Cpu {
    let (mut cpu, mut bus) = setup(&[opcode]);
    cpu.regs.a = a;
    cpu.regs.set_flag(Flag::Carry, carry_in);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    cpu
}

#[test]
fn rlca_and_rrca_rotate_8_bits() {
    let cpu = a_op(0x07, 0b1000_0101, false); // RLCA
    assert_eq!(cpu.regs.a, 0b0000_1011);
    assert_eq!(flags(&cpu), (false, false, false, true));

    let cpu = a_op(0x0F, 0b0000_0001, false); // RRCA
    assert_eq!(cpu.regs.a, 0b1000_0000);
    assert_eq!(flags(&cpu), (false, false, false, true));
}

#[test]
fn rla_and_rra_rotate_through_carry() {
    // RLA: C lama masuk ke bit 0, bit 7 keluar ke C.
    let cpu = a_op(0x17, 0b1001_0101, true);
    assert_eq!(cpu.regs.a, 0b0010_1011);
    assert!(cpu.regs.flag(Flag::Carry));

    // RRA: C lama masuk ke bit 7, bit 0 keluar ke C.
    let cpu = a_op(0x1F, 0b1000_0001, false);
    assert_eq!(cpu.regs.a, 0b0100_0000);
    assert!(cpu.regs.flag(Flag::Carry));
}

#[test]
fn rotate_a_never_sets_zero() {
    // RLA dengan A = 0x80, C = 0 → hasil 0x00, tetapi Z tetap mati.
    let cpu = a_op(0x17, 0x80, false);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (false, false, false, true));
}

#[test]
fn cpl_scf_ccf() {
    let cpu = a_op(0x2F, 0b1010_0101, false); // CPL
    assert_eq!(cpu.regs.a, 0b0101_1010);
    assert_eq!(flags(&cpu), (false, true, true, false));

    let cpu = a_op(0x37, 0x00, false); // SCF
    assert_eq!(flags(&cpu), (false, false, false, true));

    let cpu = a_op(0x3F, 0x00, true); // CCF: 1 → 0
    assert!(!cpu.regs.flag(Flag::Carry));
    let cpu = a_op(0x3F, 0x00, false); // CCF: 0 → 1
    assert!(cpu.regs.flag(Flag::Carry));
}

/// Menjalankan `LD A,a ; <op> b ; DAA` dan mengembalikan CPU-nya.
fn daa_after(op: u8, a: u8, b: u8) -> Cpu {
    let (mut cpu, mut bus) = setup(&[0x3E, a, op, b, 0x27]);
    for _ in 0..3 {
        step(&mut cpu, &mut bus);
    }
    cpu
}

#[test]
fn daa_after_addition() {
    // Desimal 15 + 27 = 42.
    let cpu = daa_after(0xC6, 0x15, 0x27);
    assert_eq!(cpu.regs.a, 0x42);
    assert!(!cpu.regs.flag(Flag::Carry));

    // 99 + 1 = 100 → "00" dengan carry (ratusan).
    let cpu = daa_after(0xC6, 0x99, 0x01);
    assert_eq!(cpu.regs.a, 0x00);
    assert_eq!(flags(&cpu), (true, false, false, true));

    // 9 + 9 = 18: H menyala setelah ADD (0x9 + 0x9 = 0x12).
    let cpu = daa_after(0xC6, 0x09, 0x09);
    assert_eq!(cpu.regs.a, 0x18);
}

#[test]
fn daa_after_subtraction() {
    // Desimal 42 − 15 = 27.
    let cpu = daa_after(0xD6, 0x42, 0x15);
    assert_eq!(cpu.regs.a, 0x27);
    assert!(cpu.regs.flag(Flag::Subtract)); // N dibiarkan apa adanya

    // 10 − 1 = 9.
    let cpu = daa_after(0xD6, 0x10, 0x01);
    assert_eq!(cpu.regs.a, 0x09);

    // 0 − 1 = "99" dengan pinjam.
    let cpu = daa_after(0xD6, 0x00, 0x01);
    assert_eq!(cpu.regs.a, 0x99);
    assert!(cpu.regs.flag(Flag::Carry));
}

// ---------------------------------------------------------------------------
// RST
// ---------------------------------------------------------------------------

#[test]
fn rst_calls_fixed_addresses() {
    for (n, opcode) in [0xC7u8, 0xCF, 0xD7, 0xDF, 0xE7, 0xEF, 0xF7, 0xFF]
        .into_iter()
        .enumerate()
    {
        let (mut cpu, mut bus) = setup(&[opcode]);
        cpu.regs.sp = 0xFFFE;
        assert_eq!(step(&mut cpu, &mut bus), 16);
        assert_eq!(usize::from(cpu.regs.pc), n * 8, "RST {opcode:#04X}");
        // Alamat kembali = 0x0101 (setelah RST yang 1 byte).
        assert_eq!((bus.read(0xFFFD), bus.read(0xFFFC)), (0x01, 0x01));
    }
}

// ---------------------------------------------------------------------------
// Prefix CB
// ---------------------------------------------------------------------------

/// Menjalankan `CB cb` dengan B = `b` dan flag C awal tertentu (8 siklus).
fn cb_b(cb: u8, b: u8, carry_in: bool) -> Cpu {
    let (mut cpu, mut bus) = setup(&[0xCB, cb]);
    cpu.regs.b = b;
    cpu.regs.set_flag(Flag::Carry, carry_in);
    assert_eq!(step(&mut cpu, &mut bus), 8);
    assert_eq!(cpu.regs.pc, START + 2);
    cpu
}

#[test]
fn cb_rlc_and_rrc_rotate_8_bits() {
    let cpu = cb_b(0x00, 0b1000_0101, false); // RLC B
    assert_eq!(cpu.regs.b, 0b0000_1011);
    assert_eq!(flags(&cpu), (false, false, false, true));

    let cpu = cb_b(0x08, 0b0000_0010, true); // RRC B
    assert_eq!(cpu.regs.b, 0b0000_0001);
    assert_eq!(flags(&cpu), (false, false, false, false));
}

#[test]
fn cb_rotate_of_zero_sets_zero_unlike_rlca() {
    // CB RLC A dengan A = 0 → Z menyala.
    let (mut cpu, mut bus) = setup(&[0xCB, 0x07]);
    cpu.regs.a = 0;
    step(&mut cpu, &mut bus);
    assert!(cpu.regs.flag(Flag::Zero));

    // RLCA (tanpa prefix) dengan A = 0 → Z tetap mati.
    let (mut cpu, mut bus) = setup(&[0x07]);
    cpu.regs.a = 0;
    step(&mut cpu, &mut bus);
    assert!(!cpu.regs.flag(Flag::Zero));
}

#[test]
fn cb_rl_and_rr_rotate_through_carry() {
    let cpu = cb_b(0x10, 0b1000_0000, false); // RL B: 9 bit lewat C
    assert_eq!(cpu.regs.b, 0x00);
    assert_eq!(flags(&cpu), (true, false, false, true));

    let cpu = cb_b(0x10, 0b0000_0001, true); // RL B: C lama masuk bit 0
    assert_eq!(cpu.regs.b, 0b0000_0011);
    assert!(!cpu.regs.flag(Flag::Carry));

    let cpu = cb_b(0x18, 0b0000_0000, true); // RR B: C lama masuk bit 7
    assert_eq!(cpu.regs.b, 0b1000_0000);
    assert_eq!(flags(&cpu), (false, false, false, false));
}

#[test]
fn cb_sla_shifts_in_zero() {
    let cpu = cb_b(0x20, 0b1000_0001, true); // SLA B
    assert_eq!(cpu.regs.b, 0b0000_0010); // carry lama TIDAK masuk
    assert!(cpu.regs.flag(Flag::Carry));

    let cpu = cb_b(0x20, 0b1000_0000, false);
    assert_eq!(cpu.regs.b, 0);
    assert_eq!(flags(&cpu), (true, false, false, true));
}

#[test]
fn cb_sra_keeps_sign_and_srl_does_not() {
    // SRA: bit 7 tetap (bilangan negatif tetap negatif).
    let cpu = cb_b(0x28, 0b1000_0001, false);
    assert_eq!(cpu.regs.b, 0b1100_0000);
    assert!(cpu.regs.flag(Flag::Carry));

    // SRL: bit 7 diisi 0.
    let cpu = cb_b(0x38, 0b1000_0001, false);
    assert_eq!(cpu.regs.b, 0b0100_0000);
    assert!(cpu.regs.flag(Flag::Carry));

    // Kasus batas: 0x01 → 0x00, Z dan C menyala untuk keduanya.
    for cb in [0x28, 0x38] {
        let cpu = cb_b(cb, 0x01, false);
        assert_eq!(cpu.regs.b, 0x00);
        assert_eq!(flags(&cpu), (true, false, false, true), "CB {cb:#04X}");
    }
}

#[test]
fn cb_swap_exchanges_nibbles_and_clears_carry() {
    let cpu = cb_b(0x30, 0xF1, true); // SWAP B
    assert_eq!(cpu.regs.b, 0x1F);
    assert_eq!(flags(&cpu), (false, false, false, false));

    let cpu = cb_b(0x30, 0x00, true);
    assert_eq!(flags(&cpu), (true, false, false, false));
}

#[test]
fn cb_bit_tests_each_bit_and_keeps_carry() {
    // B = 0b1010_1010: bit ganjil menyala, bit genap mati.
    for bit in 0..8u8 {
        let cb = 0x40 | (bit << 3); // BIT bit,B
        let cpu = cb_b(cb, 0b1010_1010, true);
        let is_set = bit % 2 == 1;
        // Z = bit itu MATI; N = 0; H = 1; C tidak berubah (tetap 1).
        assert_eq!(flags(&cpu), (!is_set, false, true, true), "BIT {bit},B");
        assert_eq!(cpu.regs.b, 0b1010_1010, "BIT tidak mengubah register");
    }
}

#[test]
fn cb_res_and_set_change_one_bit_without_flags() {
    let cpu = cb_b(0xB8, 0xFF, false); // RES 7,B
    assert_eq!(cpu.regs.b, 0x7F);
    let cpu = cb_b(0xC0, 0x00, false); // SET 0,B
    assert_eq!(cpu.regs.b, 0x01);

    // RES dan SET tidak menyentuh flag sama sekali.
    let (mut cpu, mut bus) = setup(&[0xCB, 0x80, 0xCB, 0xC0]); // RES 0,B ; SET 0,B
    cpu.regs.set_f(0xF0);
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.regs.f(), 0xF0);
}

#[test]
fn cb_hl_variants_use_memory_and_take_longer() {
    // LD HL,0xC000 ; SET 3,(HL) ; BIT 3,(HL) ; SWAP (HL)
    let (mut cpu, mut bus) = setup(&[0x21, 0x00, 0xC0, 0xCB, 0xDE, 0xCB, 0x5E, 0xCB, 0x36]);
    bus.write(0xC000, 0x20);
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus), 16); // SET (HL): baca + tulis
    assert_eq!(bus.read(0xC000), 0x28);
    assert_eq!(step(&mut cpu, &mut bus), 12); // BIT (HL): hanya baca
    assert!(!cpu.regs.flag(Flag::Zero));
    assert_eq!(step(&mut cpu, &mut bus), 16);
    assert_eq!(bus.read(0xC000), 0x82);
}

#[test]
fn every_cb_opcode_runs_with_documented_cycles() {
    // Pan Docs: CB r = 8, CB (HL) = 16, kecuali BIT n,(HL) = 12.
    for cb in 0..=255u8 {
        let (mut cpu, mut bus) = setup(&[0xCB, cb]);
        cpu.regs.set_hl(0xC000);
        let uses_hl = cb & 0b111 == 6;
        let is_bit = cb >> 6 == 1;
        let expected = match (uses_hl, is_bit) {
            (false, _) => 8,
            (true, true) => 12,
            (true, false) => 16,
        };
        assert_eq!(cpu.step(&mut bus), Ok(expected), "CB {cb:#04X}");
    }
}

// ---------------------------------------------------------------------------
// DI / EI (saklar IME). Lompatan ke interrupt menyusul di M3.
// ---------------------------------------------------------------------------

#[test]
fn ime_starts_disabled() {
    assert!(!Cpu::new().interrupts_enabled());
}

#[test]
fn ei_takes_effect_after_the_next_instruction() {
    // EI ; NOP ; NOP
    let (mut cpu, mut bus) = setup(&[0xFB, 0x00, 0x00]);
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert!(!cpu.interrupts_enabled(), "belum menyala tepat setelah EI");
    step(&mut cpu, &mut bus);
    assert!(
        cpu.interrupts_enabled(),
        "menyala setelah instruksi berikutnya"
    );
}

#[test]
fn di_right_after_ei_cancels_it() {
    // EI ; DI ; NOP → IME tetap mati.
    let (mut cpu, mut bus) = setup(&[0xFB, 0xF3, 0x00]);
    for _ in 0..3 {
        step(&mut cpu, &mut bus);
        assert!(!cpu.interrupts_enabled());
    }
}

#[test]
fn di_disables_immediately() {
    // EI ; NOP ; DI
    let (mut cpu, mut bus) = setup(&[0xFB, 0x00, 0xF3]);
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert!(cpu.interrupts_enabled());
    assert_eq!(step(&mut cpu, &mut bus), 4);
    assert!(!cpu.interrupts_enabled());
}

#[test]
fn reti_returns_and_enables_ime_immediately() {
    // RETI dengan alamat kembali 0x4000 di stack.
    let (mut cpu, mut bus) = setup(&[0xD9]);
    cpu.regs.sp = 0xC000;
    bus.write(0xC000, 0x00);
    bus.write(0xC001, 0x40);
    assert_eq!(step(&mut cpu, &mut bus), 16);
    assert_eq!((cpu.regs.pc, cpu.regs.sp), (0x4000, 0xC002));
    // Berbeda dengan EI: langsung menyala, tanpa menunggu instruksi berikutnya.
    assert!(cpu.interrupts_enabled());
}
