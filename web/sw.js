// Service worker: emulator tetap jalan tanpa internet (M8, ADR-0017).
//
// Saat pertama dipasang, semua berkas aplikasi disimpan ke cache: halaman,
// JS, .wasm, ROM demo, ikon. Setelah itu:
//   - halaman (navigasi): jaringan dulu, cache kalau offline — pengguna yang
//     online selalu mendapat versi terbaru;
//   - berkas lain: cache dulu — aman, karena URL-nya memuat versi build
//     (?v=…, ADR-0009), jadi isi URL yang sama tidak pernah berubah.
// Nama cache memuat versi build; cache build lama dihapus saat aktif.
//
// __BUILD__ dan __ROMS__ diisi oleh tools/build-web.sh (.ps1).

const BUILD = '__BUILD__';
const CACHE = `mxm-gameboy-${BUILD}`;
const ROMS = __ROMS__;

const FILES = [
  './',
  'index.html',
  `style.css?v=${BUILD}`,
  `main.js?v=${BUILD}`,
  `input.js?v=${BUILD}`,
  `save.js?v=${BUILD}`,
  `audio.js?v=${BUILD}`,
  `debugger.js?v=${BUILD}`,
  `pkg/gb_web.js?v=${BUILD}`,
  `pkg/gb_web_bg.wasm?v=${BUILD}`,
  'manifest.webmanifest',
  'icons/icon-192.png',
  'icons/icon-512.png',
  ...ROMS.map((rom) => `roms/${rom}?v=${BUILD}`),
];

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(FILES)));
  self.skipWaiting(); // build baru langsung dipakai, tidak menunggu tab ditutup
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(
        names
          .filter((name) => name.startsWith('mxm-gameboy-') && name !== CACHE)
          .map((name) => caches.delete(name)),
      ))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET' || new URL(request.url).origin !== location.origin) return;
  if (request.mode === 'navigate') {
    event.respondWith(fetch(request).catch(() => caches.match('./', { cacheName: CACHE })));
    return;
  }
  event.respondWith(caches.match(request).then((hit) => hit ?? fetch(request)));
});
