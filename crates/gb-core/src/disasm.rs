//! Disassembler: byte opcode → teks instruksi yang bisa dibaca manusia.
//!
//! Dipakai debugger (M8) untuk menampilkan "apa yang akan dijalankan CPU".
//! Decode-nya mengikuti pola bit yang sama dengan [`crate::cpu`]
//! (ADR-0010): opcode = `xx yyy zzz`, dengan `y` = `pp q`. Tes memastikan
//! panjang setiap instruksi sama dengan seberapa jauh CPU memajukan PC.
//!
//! Teks memakai gaya RGBDS, assembler Game Boy yang umum: `LD A,(HL+)`,
//! `JR NZ,$0150`, `LDH ($44),A`. Lompatan relatif ditulis sebagai alamat
//! tujuan, bukan offset, supaya langsung terbaca.

use alloc::format;
use alloc::string::String;

/// Satu instruksi hasil disassembly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// Teks, mis. `LD A,$3C`.
    pub text: String,
    /// Panjang dalam byte (1–3), termasuk prefix `CB`.
    pub length: u8,
}

/// Nama register 8-bit menurut indeks 0–7 (ADR-0010).
const R: [&str; 8] = ["B", "C", "D", "E", "H", "L", "(HL)", "A"];
/// Pasangan register untuk LD/INC/DEC/ADD HL.
const RP: [&str; 4] = ["BC", "DE", "HL", "SP"];
/// Pasangan register untuk PUSH/POP.
const RP2: [&str; 4] = ["BC", "DE", "HL", "AF"];
/// Kondisi lompatan.
const CC: [&str; 4] = ["NZ", "Z", "NC", "C"];
/// Operasi ALU dengan A.
const ALU: [&str; 8] = [
    "ADD A,", "ADC A,", "SUB ", "SBC A,", "AND ", "XOR ", "OR ", "CP ",
];
/// Operasi tanpa operan di blok 0, kolom 7.
const ACC: [&str; 8] = ["RLCA", "RRCA", "RLA", "RRA", "DAA", "CPL", "SCF", "CCF"];
/// Rotasi/geser prefix CB.
const ROT: [&str; 8] = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SWAP", "SRL"];
/// Bentuk `(BC)`, `(DE)`, `(HL+)`, `(HL-)` untuk LD tak langsung.
const INDIRECT: [&str; 4] = ["(BC)", "(DE)", "(HL+)", "(HL-)"];

/// Membaca instruksi di `address`. `read` dipanggil untuk tiap byte, jadi
/// disassembler tidak mengubah apa pun (tidak memakai bus yang bisa ditulis).
pub fn disassemble(read: impl Fn(u16) -> u8, address: u16) -> Instruction {
    let opcode = read(address);
    let d8 = read(address.wrapping_add(1));
    let d16 = u16::from_le_bytes([d8, read(address.wrapping_add(2))]);
    // Tujuan lompatan relatif: alamat setelah instruksi + offset bertanda.
    let target = address
        .wrapping_add(2)
        .wrapping_add_signed(i16::from(d8.cast_signed()));
    let (y, z) = (usize::from((opcode >> 3) & 7), usize::from(opcode & 7));
    let p = y >> 1;

    let (text, length) = match opcode {
        0x00..=0x3F => block0(opcode, d8, d16, target),
        // --- Blok 1: LD r,r (kecuali 0x76) ---
        0x76 => (String::from("HALT"), 1),
        0x40..=0x7F => (format!("LD {},{}", R[y], R[z]), 1),
        // --- Blok 2: ALU A,r ---
        0x80..=0xBF => (format!("{}{}", ALU[y], R[z]), 1),
        // --- Blok 3 ---
        0xC3 => (format!("JP ${d16:04X}"), 3),
        0xC9 => (String::from("RET"), 1),
        0xD9 => (String::from("RETI"), 1),
        0xCD => (format!("CALL ${d16:04X}"), 3),
        0xE9 => (String::from("JP HL"), 1),
        0xF9 => (String::from("LD SP,HL"), 1),
        0xE0 => (format!("LDH (${d8:02X}),A"), 2),
        0xF0 => (format!("LDH A,(${d8:02X})"), 2),
        0xE2 => (String::from("LD (C),A"), 1),
        0xF2 => (String::from("LD A,(C)"), 1),
        0xEA => (format!("LD (${d16:04X}),A"), 3),
        0xFA => (format!("LD A,(${d16:04X})"), 3),
        0xE8 => (format!("ADD SP,{}", d8.cast_signed()), 2),
        0xF8 => (format!("LD HL,SP{:+}", d8.cast_signed()), 2),
        0xF3 => (String::from("DI"), 1),
        0xFB => (String::from("EI"), 1),
        0xCB => (cb(d8), 2),
        0xC0 | 0xC8 | 0xD0 | 0xD8 => (format!("RET {}", CC[y]), 1),
        0xC2 | 0xCA | 0xD2 | 0xDA => (format!("JP {},${d16:04X}", CC[y]), 3),
        0xC4 | 0xCC | 0xD4 | 0xDC => (format!("CALL {},${d16:04X}", CC[y]), 3),
        0xC1 | 0xD1 | 0xE1 | 0xF1 => (format!("POP {}", RP2[p]), 1),
        0xC5 | 0xD5 | 0xE5 | 0xF5 => (format!("PUSH {}", RP2[p]), 1),
        0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
            (format!("{}${d8:02X}", ALU[y]), 2)
        }
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => (format!("RST ${:02X}", y * 8), 1),
        // Opcode yang tidak ada di SM83 (D3, DB, DD, E3, E4, EB, EC, ED, F4,
        // FC, FD): ditulis sebagai data mentah.
        _ => (format!("DB ${opcode:02X}"), 1),
    };
    Instruction { text, length }
}

/// Blok 0 (0x00–0x3F), dibagi menurut kolom `z` dan bit `q` seperti di CPU.
fn block0(opcode: u8, d8: u8, d16: u16, target: u16) -> (String, u8) {
    let y = usize::from((opcode >> 3) & 7);
    let (p, q) = (y >> 1, y & 1);
    match (opcode & 7, q) {
        (0, _) => match y {
            0 => (String::from("NOP"), 1),
            1 => (format!("LD (${d16:04X}),SP"), 3),
            2 => (String::from("STOP"), 2),
            3 => (format!("JR ${target:04X}"), 2),
            _ => (format!("JR {},${target:04X}", CC[y - 4]), 2),
        },
        (1, 0) => (format!("LD {},${d16:04X}", RP[p]), 3),
        (1, _) => (format!("ADD HL,{}", RP[p]), 1),
        (2, 0) => (format!("LD {},A", INDIRECT[p]), 1),
        (2, _) => (format!("LD A,{}", INDIRECT[p]), 1),
        (3, 0) => (format!("INC {}", RP[p]), 1),
        (3, _) => (format!("DEC {}", RP[p]), 1),
        (4, _) => (format!("INC {}", R[y]), 1),
        (5, _) => (format!("DEC {}", R[y]), 1),
        (6, _) => (format!("LD {},${d8:02X}", R[y]), 2),
        _ => (String::from(ACC[y]), 1),
    }
}

/// Instruksi prefix `CB` (byte kedua = `opcode`).
fn cb(opcode: u8) -> String {
    let (y, z) = (usize::from((opcode >> 3) & 7), usize::from(opcode & 7));
    match opcode >> 6 {
        0 => format!("{} {}", ROT[y], R[z]),
        1 => format!("BIT {y},{}", R[z]),
        2 => format!("RES {y},{}", R[z]),
        _ => format!("SET {y},{}", R[z]),
    }
}

#[cfg(test)]
mod tests;
