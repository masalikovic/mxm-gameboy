// Suara: sampel dari gb-core → Web Audio (M7, ADR-0015).
//
// gb-core membuat sampel stereo pada laju yang kita minta (laju AudioContext,
// biasanya 48 kHz). Modul ini hanya memutarnya: setiap frame emulator,
// sampelnya dibungkus menjadi satu AudioBuffer kecil (±800 sampel) dan
// dijadwalkan tepat setelah potongan sebelumnya.
//
// Sengaja tidak memakai AudioWorklet: AudioWorklet butuh "secure context"
// (https atau localhost), jadi tidak jalan saat menguji dari laptop lewat
// http://192.168.x.x. Penjadwalan buffer jalan di mana saja.

const LATENCY = 0.06; // detik antrean saat mulai/ulang: cukup untuk HP yang tersendat
const MAX_AHEAD = 0.25; // antrean lebih dari ini → sampel dibuang (suara tidak tertinggal)
const VOLUME = 0.6;

export function createAudio() {
  let context = null;
  let output = null;
  let nextTime = 0;
  let enabled = false;

  /** Menyalakan suara. Harus dipanggil dari sentuhan/klik (aturan autoplay browser). */
  async function enable() {
    if (!context) {
      const Context = window.AudioContext || window.webkitAudioContext;
      if (!Context) throw new Error('Browser ini tidak punya Web Audio');
      context = new Context();
      output = context.createGain();
      output.gain.value = VOLUME;
      output.connect(context.destination);
    }
    await context.resume();
    nextTime = 0;
    enabled = true;
  }

  function disable() {
    enabled = false;
    context?.suspend();
  }

  /** Menjadwalkan sampel dari takeSamples: paruh pertama kiri, paruh kedua kanan. */
  function push(samples) {
    if (!enabled || samples.length < 2) return;
    const now = context.currentTime;
    if (nextTime < now) nextTime = now + LATENCY; // antrean habis: mulai lagi dengan jeda
    if (nextTime > now + MAX_AHEAD) return; // terlalu jauh di depan: buang
    const frames = samples.length / 2;
    const buffer = context.createBuffer(2, frames, context.sampleRate);
    // Salinan per kanal oleh browser (kode native), bukan perulangan JS.
    buffer.copyToChannel(samples.subarray(0, frames), 0);
    buffer.copyToChannel(samples.subarray(frames), 1);
    const source = context.createBufferSource();
    source.buffer = buffer;
    source.connect(output);
    source.start(nextTime);
    nextTime += frames / context.sampleRate;
  }

  return {
    enable,
    disable,
    push,
    get enabled() { return enabled; },
    get rate() { return enabled ? context.sampleRate : 0; },
  };
}
