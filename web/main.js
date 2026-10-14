// Jembatan halaman ↔ WebAssembly.
//
// `./pkg/gb_web.js` dibuat oleh wasm-bindgen (tools/build-web.sh) dan tidak
// di-commit. `Emulator` berasal dari crates/gb-web, yang meneruskan panggilan
// ke gb-core. JavaScript di sini hanya mengatur waktu dan menggambar.
//
// Penanda versi pada URL di bawah diganti versi build oleh tools/build-web.sh (ADR-0009):
// URL berubah setiap build, jadi browser tidak memakai file lama dari cache.
import init, { coreVersion, Emulator } from './pkg/gb_web.js?v=__BUILD__';
import { attachInput } from './input.js?v=__BUILD__';
import { deleteSave, loadSave, saveKey, storeSave } from './save.js?v=__BUILD__';
import { createAudio } from './audio.js?v=__BUILD__';
import { createDebugger } from './debugger.js?v=__BUILD__';

const wasmUrl = new URL('./pkg/gb_web_bg.wasm?v=__BUILD__', import.meta.url);
// ROM demo: dibuild dari homebrew/<nama>/main.c di CI (ADR-0006).
const demoUrl = (file) => new URL(`./roms/${file}?v=__BUILD__`, import.meta.url);

// Versi build yang sedang tampil, supaya mudah dicek di HP tanpa membuka
// kode sumber. Contoh: "14bd9ac" (hash commit), atau "14bd9ac+lokal" kalau
// dibuild dengan perubahan yang belum di-commit.
const [commit, localStamp] = '__BUILD__'.split('-');
const buildLabel = commit.slice(0, 7) + (localStamp ? '+lokal' : '');

const WIDTH = 160;
const HEIGHT = 144;
// Satu frame Game Boy = 70.224 siklus pada 4.194.304 Hz ≈ 16,74 ms.
const FRAME_MS = (70224 / 4194304) * 1000;

const canvas = document.getElementById('lcd');
const context = canvas.getContext('2d');
const image = context.createImageData(WIDTH, HEIGHT);
const screenText = document.getElementById('screen-text');
const status = document.getElementById('status');
const build = document.getElementById('build');
const perf = document.getElementById('perf');
const romInput = document.getElementById('rom-input');
const demo = document.getElementById('demo');
const saveLine = document.getElementById('save');
const saveText = document.getElementById('save-text');
const saveDelete = document.getElementById('save-delete');
const soundButton = document.getElementById('sound');

// Suara (M7). Mati saat halaman dibuka: browser hanya mengizinkan bunyi
// setelah pengguna menyentuh halaman. Pilihan "nyala" diingat.
const audio = createAudio();
const SOUND_PREF = 'mxm-gameboy:suara';

// Sentuhan + keyboard → bit tombol → gb-core (lewat gb-web).
const input = attachInput({
  dpad: document.getElementById('dpad'),
  buttons: [...document.querySelectorAll('.pad button[data-button]')],
  onChange: (mask) => emulator?.setButtons(mask),
});

/** Palet 4 tingkat dari CSS (--lcd-0 gelap … --lcd-3 terang, ADR-0008). */
function readPalette() {
  const css = getComputedStyle(document.documentElement);
  const rgb = (name) => {
    const hex = css.getPropertyValue(name).trim().replace('#', '');
    return [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
  };
  // Framebuffer: 0 = terang … 3 = gelap. CSS: --lcd-3 = terang … --lcd-0 = gelap.
  return [rgb('--lcd-3'), rgb('--lcd-2'), rgb('--lcd-1'), rgb('--lcd-0')];
}
const palette = readPalette();

let emulator = null;
let running = false; // ROM terpasang dan tidak ada error
let paused = false; // dijeda dari debugger
let lastTime = 0;
let backlog = 0;
let emulationMs = 0;
let soundMs = 0;
let framesSinceReport = 0;

// Simpan permainan (M6). Hanya untuk cartridge berbaterai.
let current = null; // { bytes, name } ROM yang sedang jalan, untuk "Hapus simpanan"
let key = null; // kunci localStorage; null = cartridge tanpa baterai
let savePending = false;
let lastSave = 0;
const SAVE_EVERY_MS = 1000; // game yang menulis RAM tiap frame tetap disimpan ≤ 1×/detik

function showSave(text) {
  saveText.textContent = text;
  saveLine.hidden = key === null;
}

/** Menulis RAM ke penyimpanan kalau ada perubahan yang belum disimpan. */
function flushSave() {
  if (!savePending || key === null || !emulator) return;
  savePending = false;
  lastSave = performance.now();
  const time = new Date().toLocaleTimeString('id-ID', { hour: '2-digit', minute: '2-digit' });
  showSave(storeSave(key, emulator.ram())
    ? `Simpanan: tersimpan di perangkat ini (${time})`
    : 'Simpanan: browser menolak menyimpan (mode privat?)');
}

/** Framebuffer (tingkat 0–3) → piksel RGBA di canvas. */
function draw() {
  const shades = emulator.framebuffer();
  const data = image.data;
  for (let i = 0; i < shades.length; i++) {
    const [r, g, b] = palette[shades[i]];
    data[i * 4] = r;
    data[i * 4 + 1] = g;
    data[i * 4 + 2] = b;
    data[i * 4 + 3] = 255;
  }
  context.putImageData(image, 0, 0);
}

/**
 * Dipanggil browser setiap refresh layar (60/90/120 Hz). Game Boy berjalan
 * ±59,7 frame/detik, jadi jumlah frame yang dijalankan dihitung dari waktu
 * nyata yang lewat, bukan dari jumlah refresh layar HP.
 */
function tick(now) {
  if (!running) return;
  if (paused) {
    lastTime = now; // waktu jeda tidak "dikejar" saat dilanjutkan
    requestAnimationFrame(tick);
    return;
  }
  backlog += Math.min(now - lastTime, 100); // tab yang tertidur tidak "mengejar"
  lastTime = now;
  let ran = 0;
  while (backlog >= FRAME_MS && ran < 3) {
    const start = performance.now();
    try {
      emulator.runFrame();
    } catch (error) {
      stop(`Emulator berhenti: ${error}`);
      return;
    }
    const emulated = performance.now();
    emulationMs = emulationMs * 0.9 + (emulated - start) * 0.1;
    if (audio.enabled) {
      // Diukur terpisah: biaya menyerahkan sampel ke Web Audio.
      audio.push(emulator.takeSamples());
      soundMs = soundMs * 0.9 + (performance.now() - emulated) * 0.1;
    }
    backlog -= FRAME_MS;
    ran++;
  }
  if (ran > 0) {
    if (key !== null && emulator.takeRamDirty()) savePending = true;
    if (savePending && now - lastSave >= SAVE_EVERY_MS) flushSave();
    draw();
    framesSinceReport += ran;
    if (framesSinceReport % 15 < ran) debug.render(emulator, paused);
    if (framesSinceReport >= 30) {
      framesSinceReport = 0;
      const sound = audio.enabled ? ` · suara ${soundMs.toFixed(1)} ms` : '';
      perf.textContent = `emulasi ${emulationMs.toFixed(1)} ms/frame${sound} · target < 8 ms`;
    }
  }
  requestAnimationFrame(tick);
}

function stop(message) {
  running = false;
  screenText.textContent = message;
  screenText.hidden = false;
  status.textContent = message;
}

/** Memasang ROM dan mulai berjalan. */
function start(bytes, name) {
  flushSave(); // jangan sampai simpanan ROM sebelumnya hilang
  running = false;
  emulator?.free();
  key = null;
  try {
    emulator = new Emulator(bytes);
  } catch (error) {
    emulator = null;
    stop(`${error}`);
    return;
  }
  emulator.setButtons(input.mask); // tombol yang sudah ditahan sebelum ROM dimuat
  emulator.setSampleRate(audio.rate); // 0 kalau suara mati: core tidak membuat sampel
  current = { bytes, name };
  if (emulator.hasBattery()) {
    key = saveKey(bytes, emulator.title());
    const saved = loadSave(key);
    if (saved) emulator.loadRam(saved); // sebelum frame pertama, seperti baterai
    showSave(saved ? 'Simpanan: dimuat dari perangkat ini' : 'Simpanan: belum ada');
  } else {
    showSave('');
  }
  screenText.hidden = true;
  status.textContent = `ROM: ${name}`;
  paused = false;
  debug.render(emulator, paused);
  running = true;
  backlog = 0;
  lastTime = performance.now();
  requestAnimationFrame(tick);
}

romInput.addEventListener('change', async () => {
  const file = romInput.files?.[0];
  if (file) start(new Uint8Array(await file.arrayBuffer()), file.name);
});

/** Memuat salah satu ROM homebrew kita yang ikut dideploy. */
async function startDemo(file) {
  const response = await fetch(demoUrl(file));
  if (response.ok) {
    start(new Uint8Array(await response.arrayBuffer()), `${file} (demo, homebrew C)`);
  } else {
    stop('ROM demo belum ada di build ini. Muat file .gb di bawah.');
  }
}

// Debugger (M8): jeda, satu instruksi, satu frame.
const debug = createDebugger({
  panel: document.getElementById('debugger'),
  registersView: document.getElementById('dbg-registers'),
  codeView: document.getElementById('dbg-code'),
  buttons: {
    pause: document.getElementById('dbg-pause'),
    step: document.getElementById('dbg-step'),
    frame: document.getElementById('dbg-frame'),
  },
  actions: {
    togglePause() {
      if (!emulator) return;
      paused = !paused;
      if (!paused) emulator.takeSamples(); // sisa sampel dari langkah manual dibuang
      status.textContent = paused ? 'Dijeda (debugger)' : `ROM: ${current?.name ?? ''}`;
      debug.render(emulator, paused);
    },
    step() {
      debugRun(() => emulator.stepInstruction());
    },
    frame() {
      debugRun(() => emulator.runFrame());
    },
    refresh() {
      debug.render(emulator, paused);
    },
  },
});

/** Satu aksi debugger saat dijeda, lalu layar dan panel digambar ulang. */
function debugRun(action) {
  if (!emulator || !paused) return;
  try {
    action();
  } catch (error) {
    stop(`Emulator berhenti: ${error}`);
    return;
  }
  draw();
  debug.render(emulator, paused);
}

function showSound() {
  soundButton.textContent = audio.enabled ? 'SUARA: NYALA' : 'SUARA: MATI';
  soundButton.setAttribute('aria-pressed', String(audio.enabled));
}

async function setSound(on) {
  if (on) {
    try {
      await audio.enable();
    } catch (error) {
      status.textContent = `Suara tidak bisa dinyalakan: ${error.message ?? error}`;
      return;
    }
  } else {
    audio.disable();
  }
  emulator?.setSampleRate(audio.rate);
  try {
    localStorage.setItem(SOUND_PREF, on ? '1' : '0');
  } catch {
    // pilihan tidak diingat; suara tetap jalan
  }
  showSound();
}

soundButton.addEventListener('click', () => {
  setSound(!audio.enabled);
  soundButton.blur(); // Enter berikutnya untuk Start, bukan tombol ini
});

// Pilihan "nyala" dari kunjungan sebelumnya: dinyalakan pada sentuhan pertama.
try {
  if (localStorage.getItem(SOUND_PREF) === '1') {
    const resume = (event) => {
      if (event.target === soundButton) return; // tombol itu sendiri yang mengurus
      window.removeEventListener('pointerdown', resume, true);
      window.removeEventListener('keydown', resume, true);
      if (!audio.enabled) setSound(true);
    };
    window.addEventListener('pointerdown', resume, true);
    window.addEventListener('keydown', resume, true);
  }
} catch {
  // tanpa localStorage: suara mulai mati, seperti biasa
}
showSound();

saveDelete.addEventListener('click', () => {
  if (key === null || !current) return;
  if (!confirm('Hapus simpanan game ini dari perangkat ini?')) return;
  deleteSave(key);
  savePending = false;
  key = null; // supaya start() tidak menyimpan ulang RAM lama
  start(current.bytes, current.name); // mulai ulang dengan RAM kosong
});

// Tab ditutup, disembunyikan, atau HP mengunci layar: simpan sekarang.
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden') flushSave();
});
window.addEventListener('pagehide', flushSave);

demo.addEventListener('change', () => {
  startDemo(demo.value);
  demo.blur(); // supaya tombol panah kembali menggerakkan game
});

try {
  await init({ module_or_path: wasmUrl }); // mengunduh dan menyiapkan .wasm
  build.textContent = `gb-core ${coreVersion()} · build ${buildLabel}`;
  await startDemo(demo.value);
} catch (error) {
  stop(`Error: ${error.message ?? error}`);
  console.error(error);
}
