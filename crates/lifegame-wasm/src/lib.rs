//! WASM bridge for lifegame-core.

use lifegame_core::{
    Boundary, Cell, Simulation,
    all_builtins, builtin_or_err,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "panic-hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WasmSimulation {
    inner: Simulation,
}

#[wasm_bindgen]
impl WasmSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32, toroidal: bool) -> Result<WasmSimulation, JsError> {
        let boundary = if toroidal { Boundary::Toroidal } else { Boundary::Fixed };
        let inner = Simulation::new(width, height, boundary)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(WasmSimulation { inner })
    }

    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    pub fn height(&self) -> u32 {
        self.inner.height()
    }

    /// `f64` で返す（u64→BigInt の Safari 互換性問題回避）。2^53 世代まで安全
    pub fn generation(&self) -> f64 {
        self.inner.generation() as f64
    }

    #[wasm_bindgen(js_name = countAlive)]
    pub fn count_alive(&self) -> u32 {
        self.inner.count_alive()
    }

    #[wasm_bindgen(js_name = boundaryIsToroidal)]
    pub fn boundary_is_toroidal(&self) -> bool {
        matches!(self.inner.boundary(), Boundary::Toroidal)
    }

    #[wasm_bindgen(js_name = setBoundary)]
    pub fn set_boundary(&mut self, toroidal: bool) {
        self.inner
            .set_boundary(if toroidal { Boundary::Toroidal } else { Boundary::Fixed });
    }

    pub fn step(&mut self) {
        self.inner.step();
    }

    #[wasm_bindgen(js_name = stepBack)]
    pub fn step_back(&mut self) -> bool {
        self.inner.step_back()
    }

    #[wasm_bindgen(js_name = toggleCell)]
    pub fn toggle_cell(&mut self, x: i32, y: i32) {
        self.inner.toggle_cell(x, y);
    }

    #[wasm_bindgen(js_name = setCell)]
    pub fn set_cell(&mut self, x: i32, y: i32, alive: bool) {
        self.inner
            .set_cell(x, y, if alive { Cell::Alive } else { Cell::Dead });
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// `seed` is optional; pass `undefined` (or omit) for non-deterministic randomization,
    /// or a `bigint` for reproducible runs.
    pub fn randomize(&mut self, density: f32, seed: Option<u64>) -> Result<(), JsError> {
        self.inner
            .randomize(density, seed)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsError> {
        self.inner
            .resize(width, height)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Pointer (linear memory offset) to the bit-packed cells buffer. JS
    /// callers should construct a fresh view via
    /// `new Uint8Array(memory.buffer, cellsPtr(), cellsLen())`.
    ///
    /// Layout: cells are stored 1 bit each, with 8 cells per byte (LSB =
    /// leftmost cell of the byte). Each row occupies `strideBytes()` bytes
    /// — always a multiple of 8 — so the byte index for cell `(x, y)` is
    /// `y * strideBytes() + (x >> 3)` and the bit within that byte is
    /// `x & 7`. The high bits of the trailing byte/word in each row are
    /// invariant zero, so iterating `0..width` per row never reads stale
    /// data even when `width` is not a multiple of 8.
    ///
    /// IMPORTANT: any mutating call (`step`, `stepBack`, `randomize`,
    /// `resize`, `clear`, `placePattern`, `setCell`, `toggleCell`) may
    /// reallocate the underlying buffer and detach previously created
    /// `Uint8Array` views. Always re-fetch `cellsPtr` / `cellsLen` and
    /// rebuild the view after any mutation. Practically: rebuild the view
    /// once per frame.
    #[wasm_bindgen(js_name = cellsPtr)]
    pub fn cells_ptr(&self) -> *const u8 {
        self.inner.cells().as_ptr()
    }

    /// Length in *bytes* of the bit-packed cells buffer.
    /// Equals `strideBytes() * height`.
    #[wasm_bindgen(js_name = cellsLen)]
    pub fn cells_len(&self) -> usize {
        self.inner.cells().len()
    }

    /// Bytes per row in the bit-packed cells buffer. Always a multiple of
    /// 8 (the underlying storage is `u64` words).
    #[wasm_bindgen(js_name = strideBytes)]
    pub fn stride_bytes(&self) -> usize {
        self.inner.stride_bytes()
    }

    #[wasm_bindgen(js_name = placePattern)]
    pub fn place_pattern(&mut self, name: &str, ox: i32, oy: i32) -> Result<(), JsError> {
        let pat = builtin_or_err(name).map_err(|e| JsError::new(&e.to_string()))?;
        self.inner
            .place_pattern(pat, ox, oy)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// 指定パターンが alive とするセルの (x,y) 一覧を Box<[u32]> で返す。
    /// [x0, y0, x1, y1, ...] のflat配列。プレビュー描画用
    #[wasm_bindgen(js_name = patternFootprint)]
    pub fn pattern_footprint(name: &str) -> Result<Box<[u32]>, JsError> {
        let pat = builtin_or_err(name).map_err(|e| JsError::new(&e.to_string()))?;
        let mut out: Vec<u32> = Vec::new();
        for y in 0..pat.height {
            for x in 0..pat.width {
                if pat.cells[(y * pat.width + x) as usize] != 0 {
                    out.push(x);
                    out.push(y);
                }
            }
        }
        Ok(out.into_boxed_slice())
    }

    #[wasm_bindgen(js_name = listPatterns)]
    pub fn list_patterns() -> Vec<String> {
        all_builtins().iter().map(|p| p.name.to_string()).collect()
    }

    /// パターン名と category slug をペアで並べたフラット配列を返す。
    /// JS 側は `[name, category, name, category, ...]` をペアで読む。
    /// 例: `["blinker", "oscillator", "glider", "spaceship", ...]`
    #[wasm_bindgen(js_name = listPatternsWithCategory)]
    pub fn list_patterns_with_category() -> Vec<String> {
        all_builtins()
            .iter()
            .flat_map(|p| [p.name.to_string(), p.category.slug().to_string()])
            .collect()
    }

    #[wasm_bindgen(js_name = setHistoryCapacity)]
    pub fn set_history_capacity(&mut self, cap: usize) {
        self.inner.set_history_capacity(cap);
    }

    #[wasm_bindgen(js_name = historyCapacity)]
    pub fn history_capacity(&self) -> usize {
        self.inner.history_capacity()
    }
}
