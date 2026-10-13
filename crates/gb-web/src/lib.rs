//! Adapter browser untuk `gb-core`.
//!
//! Crate ini tipis dengan sengaja: ia hanya menerjemahkan antara dunia
//! JavaScript (string, angka, nanti canvas dan tombol sentuh) dan API core.
//! Logika emulasi tidak boleh ditulis di sini — tempatnya di `gb-core`.
//!
//! Fungsi bertanda `#[wasm_bindgen]` bisa dipanggil dari `web/main.js`.

use wasm_bindgen::prelude::wasm_bindgen;

/// Sapaan dari core, untuk ditampilkan di halaman web.
#[wasm_bindgen]
#[must_use]
pub fn greeting() -> String {
    gb_core::greeting().into()
}

/// Versi `gb-core` yang ikut ter-build ke dalam file `.wasm` ini.
#[wasm_bindgen(js_name = coreVersion)]
#[must_use]
pub fn core_version() -> String {
    gb_core::VERSION.into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn web_shows_the_same_text_as_core() {
        assert_eq!(super::greeting(), gb_core::greeting());
        assert_eq!(super::core_version(), gb_core::VERSION);
    }
}
