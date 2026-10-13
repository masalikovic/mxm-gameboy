// Jembatan halaman ↔ WebAssembly.
//
// `./pkg/gb_web.js` dibuat oleh wasm-bindgen (tools/build-web.sh) dan tidak
// di-commit. Fungsi `greeting` dan `coreVersion` berasal dari crates/gb-web,
// yang meneruskan panggilan ke gb-core. JavaScript di sini hanya menampilkan.
//
// Penanda versi pada URL di bawah diganti versi build oleh tools/build-web.sh (ADR-0009):
// URL berubah setiap build, jadi browser tidak memakai file lama dari cache.
import init, { greeting, coreVersion } from './pkg/gb_web.js?v=__BUILD__';

const wasmUrl = new URL('./pkg/gb_web_bg.wasm?v=__BUILD__', import.meta.url);

const screenText = document.getElementById('screen-text');
const status = document.getElementById('status');

try {
  await init({ module_or_path: wasmUrl }); // mengunduh dan menyiapkan .wasm
  screenText.textContent = greeting();
  status.textContent = `gb-core ${coreVersion()} · WebAssembly aktif`;
} catch (error) {
  screenText.textContent = 'Gagal memuat emulator';
  status.textContent = `Error: ${error.message ?? error}`;
  console.error(error);
}
