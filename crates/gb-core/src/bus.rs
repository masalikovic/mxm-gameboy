//! Bus memori: satu-satunya jalan CPU berbicara dengan dunia luar.
//!
//! Bagi CPU, semua komponen terlihat sebagai alamat 16-bit (0000–FFFF):
//! cartridge, RAM, register layar, timer, tombol. CPU tidak tahu komponen apa
//! yang ada di balik sebuah alamat; ia hanya memanggil [`Bus::read`] dan
//! [`Bus::write`]. Karena itu CPU bisa diuji dengan bus palsu yang sederhana,
//! sebelum cartridge, PPU, dan timer ada (ADR-0010).

/// Sesuatu yang bisa dibaca dan ditulis CPU lewat alamat 16-bit.
pub trait Bus {
    /// Membaca satu byte.
    ///
    /// Memakai `&mut self` karena di hardware asli membaca beberapa register
    /// bisa mengubah keadaan komponen (misalnya register tombol).
    fn read(&mut self, address: u16) -> u8;

    /// Menulis satu byte.
    fn write(&mut self, address: u16, value: u8);
}

/// Bus paling sederhana untuk tes: 64 KiB RAM datar, semua alamat bisa
/// dibaca dan ditulis. Tidak ada cartridge, I/O, atau aturan khusus.
#[cfg(test)]
pub(crate) struct FlatBus {
    memory: [u8; 0x1_0000],
}

#[cfg(test)]
impl FlatBus {
    /// Bus berisi nol, dengan `program` disalin mulai alamat `start`.
    #[expect(
        clippy::large_stack_arrays,
        reason = "hanya untuk tes; 64 KiB muat di stack thread tes"
    )]
    pub(crate) fn with_program(start: u16, program: &[u8]) -> Self {
        let mut bus = Self {
            memory: [0; 0x1_0000],
        };
        let start = usize::from(start);
        bus.memory[start..start + program.len()].copy_from_slice(program);
        bus
    }
}

#[cfg(test)]
impl Bus for FlatBus {
    fn read(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}
