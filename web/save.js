// Simpan permainan: RAM cartridge berbaterai ↔ penyimpanan browser (M6).
//
// Di Game Boy asli, baterai kecil di cartridge menjaga isi RAM saat konsol
// dimatikan. Di sini perannya diganti localStorage: isi RAM disimpan di
// perangkat ini saja, tidak pernah diunggah (sama seperti ROM, ADR-0006).
//
// gb-core tidak tahu apa itu localStorage; ia hanya memberi tahu "RAM
// berubah" (takeRamDirty) dan menyerahkan isinya (ram/loadRam).

const PREFIX = 'mxm-gameboy:simpan:';

/**
 * Kunci simpanan dari ISI ROM (FNV-1a 32-bit), bukan dari judul: dua ROM
 * berbeda dengan judul sama tidak saling menimpa. Sengaja tidak memakai
 * crypto.subtle, yang tidak tersedia di http:// alamat jaringan lokal.
 */
export function saveKey(rom, title) {
  let hash = 0x811c9dc5;
  for (let i = 0; i < rom.length; i++) {
    hash ^= rom[i];
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `${PREFIX}${title || 'tanpa-judul'}:${hash.toString(16).padStart(8, '0')}`;
}

/** Uint8Array → base64 (localStorage hanya menyimpan teks). */
function toBase64(bytes) {
  let text = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

function fromBase64(text) {
  const raw = atob(text);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

// localStorage bisa dilarang (mode privat, data situs diblokir) atau penuh.
// Semua akses dibungkus try/catch: tanpa penyimpanan, game tetap jalan,
// hanya simpanannya hilang saat halaman ditutup.

/** Simpanan untuk `key`, atau null kalau belum ada / tidak bisa dibaca. */
export function loadSave(key) {
  try {
    const text = localStorage.getItem(key);
    return text === null ? null : fromBase64(text);
  } catch {
    return null;
  }
}

/** Menyimpan; mengembalikan false kalau browser menolak. */
export function storeSave(key, bytes) {
  try {
    localStorage.setItem(key, toBase64(bytes));
    return true;
  } catch {
    return false;
  }
}

export function deleteSave(key) {
  try {
    localStorage.removeItem(key);
  } catch {
    // tidak ada yang bisa dihapus
  }
}
