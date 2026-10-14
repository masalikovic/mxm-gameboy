use super::Apu;

/// Memajukan frame sequencer `steps` kali.
fn frames(apu: &mut Apu, steps: u32) {
    for _ in 0..steps {
        apu.frame_sequencer();
    }
}

/// Kanal 2 berbunyi: DAC menyala (volume 15), duty 50 %, lalu trigger.
fn trigger_ch2(apu: &mut Apu, nr24: u8) {
    apu.write(0xFF16, 0x80);
    apu.write(0xFF17, 0xF0);
    apu.write(0xFF18, 0x00);
    apu.write(0xFF19, nr24 | 0x80);
}

#[test]
fn unused_bits_read_as_one() {
    let mut apu = Apu::new();
    apu.write(0xFF13, 0x12); // NR13 hanya bisa ditulis
    assert_eq!(apu.read(0xFF13), 0xFF);
    apu.write(0xFF11, 0x80);
    assert_eq!(apu.read(0xFF11), 0xBF, "hanya duty yang terbaca");
    assert_eq!(apu.read(0xFF15), 0xFF, "alamat kosong");
    assert_eq!(apu.read(0xFF26) & 0xF0, 0xF0, "menyala + bit 6–4 = 1");
}

#[test]
fn trigger_sets_status_and_dac_off_stops_the_channel() {
    let mut apu = Apu::new();
    trigger_ch2(&mut apu, 0);
    assert_eq!(apu.read(0xFF26) & 0x0F, 0b0010);
    apu.write(0xFF17, 0x00); // DAC mati
    assert_eq!(apu.read(0xFF26) & 0x0F, 0);
}

#[test]
fn length_counter_stops_the_channel() {
    let mut apu = Apu::new();
    apu.write(0xFF16, 0x80 | 0x3C); // panjang 64 − 60 = 4
    apu.write(0xFF17, 0xF0);
    apu.write(0xFF19, 0xC0); // trigger + length menyala (langkah berikut 0)
    frames(&mut apu, 5); // langkah 0, 2, 4 memajukan length → masih 1
    assert_eq!(apu.read(0xFF26) & 0b10, 0b10);
    frames(&mut apu, 2); // langkah 6 → 0
    assert_eq!(apu.read(0xFF26) & 0b10, 0);
}

#[test]
fn enabling_length_before_an_odd_step_clocks_it_once_more() {
    let mut apu = Apu::new();
    frames(&mut apu, 1); // langkah berikut: 1 (tidak memajukan length)
    apu.write(0xFF16, 63); // panjang 1
    apu.write(0xFF17, 0xF0);
    apu.write(0xFF19, 0x80); // trigger tanpa length
    apu.write(0xFF19, 0x40); // length dinyalakan: detak tambahan → 0
    assert_eq!(apu.read(0xFF26) & 0b10, 0, "berhenti tanpa menunggu detak");
}

#[test]
fn sweep_overflow_on_trigger_stops_channel_1() {
    let mut apu = Apu::new();
    apu.write(0xFF10, 0x11); // periode 1, naik, geser 1
    apu.write(0xFF12, 0xF0);
    apu.write(0xFF13, 0xFF);
    apu.write(0xFF14, 0x87); // frekuensi 2047 + 2047/2 > 2047
    assert_eq!(apu.read(0xFF26) & 1, 0);
}

#[test]
fn sweep_changes_frequency_on_steps_2_and_6() {
    let mut apu = Apu::new();
    apu.write(0xFF10, 0x11); // periode 1, naik, geser 1
    apu.write(0xFF12, 0xF0);
    apu.write(0xFF13, 0x00);
    apu.write(0xFF14, 0x81); // frekuensi 0x100
    frames(&mut apu, 3); // langkah 0, 1, 2: sweep di langkah 2
    assert_eq!(apu.ch1.frequency, 0x180);
}

#[test]
fn envelope_lowers_volume_on_step_7() {
    let mut apu = Apu::new();
    apu.write(0xFF16, 0x80);
    apu.write(0xFF17, 0xF1); // volume 15, turun, periode 1
    apu.write(0xFF19, 0x80);
    frames(&mut apu, 8);
    assert_eq!(apu.ch2.envelope.volume, 14);
}

#[test]
fn power_off_clears_registers_but_keeps_length_and_wave_ram() {
    let mut apu = Apu::new();
    apu.write(0xFF30, 0xAB);
    apu.write(0xFF16, 0x80 | 0x0A);
    trigger_ch2(&mut apu, 0);
    apu.write(0xFF26, 0x00);
    assert_eq!(apu.read(0xFF26), 0x70, "mati, semua kanal diam");
    assert_eq!(apu.read(0xFF17), 0x00);
    apu.write(0xFF17, 0xF0);
    assert_eq!(apu.read(0xFF17), 0x00, "tulisan diabaikan saat mati");
    apu.write(0xFF16, 20); // length tetap bisa ditulis (DMG)
    assert_eq!(apu.ch2.length.counter, 44);
    assert_eq!(apu.read(0xFF30), 0xAB, "RAM gelombang tetap");
}

#[test]
fn wave_volume_code_shifts_the_sample() {
    let mut apu = Apu::new();
    apu.ch3.enabled = true;
    apu.ch3.sample = 0x0C;
    for (code, expected) in [(0, 0), (1, 0x0C), (2, 0x06), (3, 0x03)] {
        apu.ch3.volume_code = code;
        assert_eq!(apu.ch3.output(), expected);
    }
}

#[test]
fn noise_lfsr_moves() {
    let mut apu = Apu::new();
    apu.write(0xFF21, 0xF0);
    apu.write(0xFF22, 0x00); // pembagi 8, geser 0
    apu.write(0xFF23, 0x80);
    let start = apu.ch4.lfsr;
    apu.tick(16);
    assert_ne!(apu.ch4.lfsr, start);
}

#[test]
fn no_samples_until_a_rate_is_set() {
    let mut apu = Apu::new();
    for _ in 0..1000 {
        apu.tick(24);
    }
    assert!(apu.take_samples().is_empty());
}

#[test]
fn makes_stereo_samples_at_the_requested_rate() {
    let mut apu = Apu::new();
    apu.set_sample_rate(48_000);
    trigger_ch2(&mut apu, 0x07); // frekuensi tinggi, gelombang terdengar
    let mut cycles = 0u32;
    while cycles < 4_194_304 / 10 {
        apu.tick(4);
        cycles += 4;
    }
    let samples = apu.take_samples();
    assert_eq!(samples.len(), 2 * 4_800, "0,1 detik × 48 kHz × 2 kanal");
    let (min, max) = samples
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    assert!(max - min > 0.1, "ada gelombang, bukan sunyi");
    assert!(apu.take_samples().is_empty(), "sudah diambil");
}

#[test]
fn status_bits_follow_channel_numbers() {
    let mut apu = Apu::new();
    apu.write(0xFF1A, 0x80); // DAC kanal 3
    apu.write(0xFF1E, 0x80);
    assert_eq!(apu.read(0xFF26) & 0x0F, 0b0100, "kanal 3 = bit 2");
    apu.write(0xFF21, 0xF0);
    apu.write(0xFF23, 0x80);
    assert_eq!(apu.read(0xFF26) & 0x0F, 0b1100, "kanal 4 = bit 3");
}

#[test]
fn noise_7_bit_mode_also_feeds_bit_6() {
    for (nr43, expected) in [(0x00, 0x3FFF), (0x08, 0x3FBF)] {
        let mut apu = Apu::new();
        apu.write(0xFF21, 0xF0);
        apu.write(0xFF22, nr43); // pembagi 8, geser 0
        apu.write(0xFF23, 0x80);
        apu.tick(8); // satu langkah LFSR: 0x7FFF → bit baru = 1 ^ 1 = 0
        assert_eq!(apu.ch4.lfsr, expected, "NR43 = {nr43:#04x}");
    }
}

#[test]
fn nr51_routes_channels_left_and_right() {
    let mut apu = Apu::new();
    apu.set_sample_rate(48_000);
    apu.write(0xFF25, 0x20); // kanal 2 hanya ke kiri
    trigger_ch2(&mut apu, 0x07);
    for _ in 0..10_000 {
        apu.tick(4);
    }
    let samples = apu.take_samples();
    let energy = |side: usize| {
        samples
            .iter()
            .skip(side)
            .step_by(2)
            .map(|s| s.abs())
            .sum::<f32>()
    };
    assert!(energy(0) > 1.0, "kiri berbunyi");
    assert!(energy(1) < 1e-6, "kanan sunyi");
}
