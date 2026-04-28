//! Grid type and cell-level operations.

use crate::error::CoreError;
use crate::patterns::Pattern;
use rand::Rng;

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Cell {
    Dead = 0,
    Alive = 1,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Boundary {
    Toroidal,
    Fixed,
}

#[derive(Clone, Debug)]
pub struct Grid {
    width: u32,
    height: u32,
    cells: Vec<u8>,
}

impl Grid {
    pub fn new(width: u32, height: u32) -> Result<Self, CoreError> {
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidDimensions { width, height });
        }
        // Guard against dimensions that don't fit in i32: many internal
        // operations use signed arithmetic (e.g. neighbour offsets) and casting
        // a value larger than `i32::MAX` would silently wrap.
        if width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(CoreError::InvalidDimensions { width, height });
        }
        // Guard against `usize` multiplication overflow: on 32-bit / wasm32
        // targets `usize` is 32 bits wide, so `width * height` may overflow
        // even when each factor passes the `i32::MAX` check above.
        let len = (width as usize)
            .checked_mul(height as usize)
            .ok_or(CoreError::InvalidDimensions { width, height })?;
        Ok(Self {
            width,
            height,
            cells: vec![0u8; len],
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    pub fn cells_mut(&mut self) -> &mut [u8] {
        &mut self.cells
    }

    pub fn get(&self, x: i32, y: i32, boundary: Boundary) -> Cell {
        let w = self.width as i32;
        let h = self.height as i32;
        let (xi, yi) = match boundary {
            Boundary::Toroidal => {
                let xi = x.rem_euclid(w);
                let yi = y.rem_euclid(h);
                (xi as u32, yi as u32)
            }
            Boundary::Fixed => {
                if x < 0 || x >= w || y < 0 || y >= h {
                    return Cell::Dead;
                }
                (x as u32, y as u32)
            }
        };
        let idx = (yi * self.width + xi) as usize;
        if self.cells[idx] == 0 {
            Cell::Dead
        } else {
            Cell::Alive
        }
    }

    pub fn set(&mut self, x: i32, y: i32, cell: Cell) {
        let w = self.width as i32;
        let h = self.height as i32;
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        let idx = ((y as u32) * self.width + (x as u32)) as usize;
        self.cells[idx] = cell as u8;
    }

    pub fn toggle(&mut self, x: i32, y: i32) {
        let w = self.width as i32;
        let h = self.height as i32;
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        let idx = ((y as u32) * self.width + (x as u32)) as usize;
        self.cells[idx] = if self.cells[idx] == 0 { 1 } else { 0 };
    }

    pub fn clear(&mut self) {
        for c in self.cells.iter_mut() {
            *c = 0;
        }
    }

    pub fn fill_random(
        &mut self,
        density: f32,
        rng: &mut impl rand::RngCore,
    ) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&density) || density.is_nan() {
            return Err(CoreError::InvalidDensity(density));
        }
        for c in self.cells.iter_mut() {
            let r: f32 = rng.gen();
            *c = if r < density { 1 } else { 0 };
        }
        Ok(())
    }

    pub fn count_alive(&self) -> u32 {
        self.cells.iter().map(|&c| c as u32).sum()
    }

    pub fn place_pattern(
        &mut self,
        pattern: &Pattern,
        ox: i32,
        oy: i32,
        boundary: Boundary,
    ) -> Result<(), CoreError> {
        let w = self.width as i32;
        let h = self.height as i32;
        let oob = || CoreError::PatternOutOfBounds {
            name: pattern.name.to_string(),
            ox,
            oy,
            gw: self.width,
            gh: self.height,
        };
        // `pattern.width`/`height` are `u32`; cast to `i32` only after bounding
        // them by `i32::MAX` so we never depend on a wrapping cast.
        if pattern.width > i32::MAX as u32 || pattern.height > i32::MAX as u32 {
            return Err(oob());
        }
        let pw = pattern.width as i32;
        let ph = pattern.height as i32;
        // For Fixed: must fit entirely. Use checked arithmetic so extreme
        // offsets cannot overflow `i32` (panic in debug, wrap in release).
        if boundary == Boundary::Fixed {
            if ox < 0 || oy < 0 {
                return Err(oob());
            }
            let end_x = ox.checked_add(pw).ok_or_else(oob)?;
            let end_y = oy.checked_add(ph).ok_or_else(oob)?;
            if end_x > w || end_y > h {
                return Err(oob());
            }
        }
        for py in 0..pattern.height {
            for px in 0..pattern.width {
                let v = pattern.cells[(py * pattern.width + px) as usize];
                if v == 0 {
                    continue;
                }
                // `px`/`py` are bounded by `pattern.width`/`height` which we've
                // already verified fit in `i32`, so `as i32` is safe here.
                let gx = ox.checked_add(px as i32).ok_or_else(oob)?;
                let gy = oy.checked_add(py as i32).ok_or_else(oob)?;
                match boundary {
                    Boundary::Toroidal => {
                        let xi = gx.rem_euclid(w) as u32;
                        let yi = gy.rem_euclid(h) as u32;
                        let idx = (yi * self.width + xi) as usize;
                        self.cells[idx] = 1;
                    }
                    Boundary::Fixed => {
                        let idx = ((gy as u32) * self.width + (gx as u32)) as usize;
                        self.cells[idx] = 1;
                    }
                }
            }
        }
        Ok(())
    }
}
