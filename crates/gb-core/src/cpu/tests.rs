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
