use super::disassemble;
use crate::bus::FlatBus;
use crate::cpu::{Cpu, Registers};

/// Disassembly dari beberapa byte yang dimulai di 0x0100.
fn text(bytes: &[u8]) -> (alloc::string::String, u8) {
    let read = |address: u16| {
        bytes
            .get(usize::from(address.wrapping_sub(0x0100)))
            .copied()
            .unwrap_or(0)
    };
    let instruction = disassemble(read, 0x0100);
    (instruction.text, instruction.length)
}

#[test]
fn common_instructions() {
    let cases: &[(&[u8], &str, u8)] = &[
        (&[0x00], "NOP", 1),
        (&[0x3E, 0x3C], "LD A,$3C", 2),
        (&[0x21, 0x34, 0x12], "LD HL,$1234", 3),
        (&[0x2A], "LD A,(HL+)", 1),
        (&[0x36, 0x99], "LD (HL),$99", 2),
        (&[0x7E], "LD A,(HL)", 1),
        (&[0x76], "HALT", 1),
        (&[0x90], "SUB B", 1),
        (&[0x8E], "ADC A,(HL)", 1),
        (&[0xFE, 0x10], "CP $10", 2),
        (&[0xE0, 0x44], "LDH ($44),A", 2),
        (&[0xF0, 0x00], "LDH A,($00)", 2),
        (&[0xEA, 0x00, 0xC0], "LD ($C000),A", 3),
        (&[0xC5], "PUSH BC", 1),
        (&[0xF1], "POP AF", 1),
        (&[0xEF], "RST $28", 1),
        (&[0xE8, 0xFE], "ADD SP,-2", 2),
        (&[0xF8, 0x05], "LD HL,SP+5", 2),
        (&[0xD3], "DB $D3", 1),
        (&[0xCB, 0x37], "SWAP A", 2),
        (&[0xCB, 0x7C], "BIT 7,H", 2),
        (&[0xCB, 0x86], "RES 0,(HL)", 2),
        (&[0xCB, 0xFF], "SET 7,A", 2),
    ];
    for &(bytes, expected, length) in cases {
        assert_eq!(text(bytes), (expected.into(), length), "{bytes:02X?}");
    }
}

#[test]
fn jumps_show_their_destination() {
    // JR di 0x0100, panjang 2: tujuan = 0x0102 + offset.
    assert_eq!(
        text(&[0x18, 0xFE]).0,
        "JR $0100",
        "JR -2: berputar di tempat"
    );
    assert_eq!(text(&[0x20, 0x10]).0, "JR NZ,$0112");
    assert_eq!(text(&[0xC3, 0x50, 0x01]).0, "JP $0150");
    assert_eq!(text(&[0xDC, 0x00, 0x40]).0, "CALL C,$4000");
    assert_eq!(text(&[0xC8]).0, "RET Z");
}

/// Opcode yang memindahkan PC ke tempat lain (lompat, panggil, kembali)
/// atau yang tidak ada di SM83: panjangnya tidak bisa dicek lewat PC.
fn moves_pc_elsewhere(opcode: u8) -> bool {
    matches!(
        opcode,
        0x18 | 0x20
            | 0x28
            | 0x30
            | 0x38
            | 0xC0
            | 0xC2
            | 0xC3
            | 0xC4
            | 0xC7
            | 0xC8
            | 0xC9
            | 0xCA
            | 0xCC
            | 0xCD
            | 0xCF
            | 0xD0
            | 0xD2
            | 0xD4
            | 0xD7
            | 0xD8
            | 0xD9
            | 0xDA
            | 0xDC
            | 0xDF
            | 0xE7
            | 0xE9
            | 0xEF
            | 0xF7
            | 0xFF
            | 0xD3
            | 0xDB
            | 0xDD
            | 0xE3
            | 0xE4
            | 0xEB
            | 0xEC
            | 0xED
            | 0xF4
            | 0xFC
            | 0xFD
    )
}

/// Panjang menurut disassembler harus sama dengan langkah PC menurut CPU,
/// untuk setiap opcode yang tidak melompat (termasuk 256 opcode CB). Dua
/// kode yang ditulis terpisah saling memeriksa.
#[test]
fn length_matches_how_far_the_cpu_moves_pc() {
    let mut checked = 0;
    for first in 0..=0xFFu8 {
        if moves_pc_elsewhere(first) {
            continue;
        }
        let seconds: alloc::vec::Vec<u8> = if first == 0xCB {
            (0..=0xFF).collect()
        } else {
            alloc::vec![0x12]
        };
        for second in seconds {
            let program = [first, second, 0x34];
            let mut bus = FlatBus::with_program(0x0100, &program);
            let mut cpu = Cpu::new();
            cpu.regs = Registers::default();
            cpu.regs.pc = 0x0100;
            cpu.regs.sp = 0xFFFE;
            cpu.step(&mut bus).expect("opcode harus ada");
            let length = disassemble(
                |a| program.get(usize::from(a - 0x0100)).copied().unwrap_or(0),
                0x0100,
            )
            .length;
            assert_eq!(
                cpu.regs.pc - 0x0100,
                u16::from(length),
                "opcode {first:02X} {second:02X}"
            );
            checked += 1;
        }
    }
    // 256 opcode − 41 yang melompat/tidak ada − prefix CB = 214, ditambah
    // 256 opcode CB di belakang prefix.
    assert_eq!(
        checked,
        214 + 256,
        "semua opcode non-lompat + 256 opcode CB"
    );
}
