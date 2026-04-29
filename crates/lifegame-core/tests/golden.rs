//! Golden tests for lifegame-core.
//!
//! These tests pin the *observable* behaviour of the core engine so that
//! algorithmic optimisations (e.g. removing per-cell `match` on boundary,
//! bit-packing the grid, SWAR neighbour counting) cannot regress correctness
//! without flipping a test red.
//!
//! Categories (kept stable so future changes are easy to locate):
//!   A. B3/S23 rule exhaustively, by neighbour-count, both states.
//!   B. Known patterns: blinker / toad / beacon / pulsar / pentadecathlon /
//!      block / beehive / loaf still, glider 4-step displacement, exact frames.
//!   C. Toroidal boundary: wrap at every edge & corner.
//!   D. Fixed boundary: grid edge behaves as permanently-Dead neighbours.
//!   E. Degenerate shapes: 1x1, 2x2, 1xN, Nx1.
//!   F. Symmetry invariants: rotate-then-step == step-then-rotate (Toroidal).
//!   G. Long-running large board: gosper-glider-gun stays alive & grows.
//!   H. Deterministic seeded snapshot via SHA-256 hash of cells after N steps.

use lifegame_core::{
    all_builtins, builtin, next_generation, Boundary, Cell, Grid, PatternCategory, Simulation,
};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Render a list of `&str` rows ('#'/'1' = alive, anything else = dead) into
/// a `Grid` of exactly that size.
fn grid_from_rows(rows: &[&str]) -> Grid {
    let h = rows.len() as u32;
    let w = rows[0].len() as u32;
    let mut g = Grid::new(w, h).unwrap();
    for (y, row) in rows.iter().enumerate() {
        assert_eq!(row.len() as u32, w, "row {y} width mismatch");
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' || ch == '1' {
                g.set(x as i32, y as i32, Cell::Alive);
            }
        }
    }
    g
}

/// Step a single generation in isolation (no Simulation history).
fn step_once(g: &Grid, b: Boundary) -> Grid {
    let mut dst = Grid::new(g.width(), g.height()).unwrap();
    next_generation(g, &mut dst, b);
    dst
}

/// Compare two Grids by (w, h, cells) — must match exactly.
fn assert_grids_eq(actual: &Grid, expected: &Grid, ctx: &str) {
    assert_eq!(actual.width(), expected.width(), "{ctx}: width");
    assert_eq!(actual.height(), expected.height(), "{ctx}: height");
    assert_eq!(actual.cells(), expected.cells(), "{ctx}: cells differ");
}

/// Place `n` alive neighbours around the centre (1,1) of a 3x3 grid, with the
/// centre cell set according to `centre_alive`. Returns a 3x3 Grid.
///
/// Neighbour positions are encoded as a fixed order so n=0..=8 enumerates all
/// distinct subsets of size n via a bitmask.
fn make_3x3(centre_alive: bool, neighbour_mask: u8) -> Grid {
    // 8 neighbour positions in a deterministic order.
    const POS: [(i32, i32); 8] = [
        (0, 0), (1, 0), (2, 0),
        (0, 1),         (2, 1),
        (0, 2), (1, 2), (2, 2),
    ];
    let mut g = Grid::new(3, 3).unwrap();
    if centre_alive {
        g.set(1, 1, Cell::Alive);
    }
    for (i, &(x, y)) in POS.iter().enumerate() {
        if (neighbour_mask >> i) & 1 == 1 {
            g.set(x, y, Cell::Alive);
        }
    }
    g
}

/// Rotate a Grid 90 degrees clockwise.
/// Cell at (x, y) of the original ends up at (h-1-y, x) of the rotated grid.
fn rotate_cw(g: &Grid) -> Grid {
    let w = g.width();
    let h = g.height();
    let mut out = Grid::new(h, w).unwrap();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if g.get(x, y, Boundary::Fixed) == Cell::Alive {
                let nx = (h as i32 - 1) - y;
                let ny = x;
                out.set(nx, ny, Cell::Alive);
            }
        }
    }
    out
}

fn hex_hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

// ---------------------------------------------------------------------------
// A. B3/S23 rule, exhaustively
// ---------------------------------------------------------------------------

/// The centre cell of a 3x3 board with `centre_alive` and `k` alive neighbours
/// must follow B3/S23 after one generation.
///
/// We test ALL C(8, k) subsets for k = 0..=8 (so 256 placements per centre
/// state, 512 in total) — this pins the rule against any future inlining,
/// SIMD or bit-pack rewrite.
#[test]
fn rule_b3s23_centre_after_one_step() {
    for centre_alive in [false, true] {
        for mask in 0u8..=255 {
            let n = mask.count_ones();
            let g = make_3x3(centre_alive, mask);
            // Use Fixed so neighbours outside the 3x3 are guaranteed dead.
            let next = step_once(&g, Boundary::Fixed);
            let centre_next = next.get(1, 1, Boundary::Fixed) == Cell::Alive;
            let expected = match (centre_alive, n) {
                (true, 2) | (true, 3) => true,
                (false, 3) => true,
                _ => false,
            };
            assert_eq!(
                centre_next, expected,
                "centre={centre_alive} neighbours={n} mask={mask:08b}"
            );
        }
    }
}

/// Same exhaustive sweep but on a Toroidal 3x3. On a 3x3 torus *every* cell
/// is a neighbour of every other cell, so this is a different (stricter)
/// stress: it pins behaviour where edge wrapping touches the centre.
#[test]
fn rule_b3s23_toroidal_3x3_centre() {
    // On a 3x3 torus, the centre cell has the same 8 neighbours as on Fixed,
    // because wrapping does not introduce duplicates within distance 1.
    for centre_alive in [false, true] {
        for mask in 0u8..=255 {
            let n = mask.count_ones();
            let g = make_3x3(centre_alive, mask);
            let next = step_once(&g, Boundary::Toroidal);
            let centre_next = next.get(1, 1, Boundary::Toroidal) == Cell::Alive;
            let expected = matches!((centre_alive, n), (true, 2) | (true, 3) | (false, 3));
            assert_eq!(
                centre_next, expected,
                "toroidal centre={centre_alive} mask={mask:08b}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// B. Known patterns — exact bit-for-bit ground truth
// ---------------------------------------------------------------------------

#[test]
fn blinker_exact_two_phases() {
    // Phase 0: horizontal three-in-a-row.
    let g0 = grid_from_rows(&[
        ".....",
        ".....",
        ".###.",
        ".....",
        ".....",
    ]);
    let g1_expected = grid_from_rows(&[
        ".....",
        "..#..",
        "..#..",
        "..#..",
        ".....",
    ]);
    let g2_expected = g0.clone();

    let g1 = step_once(&g0, Boundary::Fixed);
    assert_grids_eq(&g1, &g1_expected, "blinker step 1");
    let g2 = step_once(&g1, Boundary::Fixed);
    assert_grids_eq(&g2, &g2_expected, "blinker step 2 (period)");
}

#[test]
fn toad_exact_two_phases() {
    let g0 = grid_from_rows(&[
        "......",
        "......",
        "..###.",
        ".###..",
        "......",
        "......",
    ]);
    // Toad period-2 phase: the canonical "second" frame.
    let g1_expected = grid_from_rows(&[
        "......",
        "...#..",
        ".#..#.",
        ".#..#.",
        "..#...",
        "......",
    ]);
    let g1 = step_once(&g0, Boundary::Fixed);
    assert_grids_eq(&g1, &g1_expected, "toad step 1");
    let g2 = step_once(&g1, Boundary::Fixed);
    assert_grids_eq(&g2, &g0, "toad step 2 (period)");
}

#[test]
fn beacon_exact_two_phases() {
    let g0 = grid_from_rows(&[
        "......",
        ".##...",
        ".##...",
        "...##.",
        "...##.",
        "......",
    ]);
    let g1_expected = grid_from_rows(&[
        "......",
        ".##...",
        ".#....",
        "....#.",
        "...##.",
        "......",
    ]);
    let g1 = step_once(&g0, Boundary::Fixed);
    assert_grids_eq(&g1, &g1_expected, "beacon step 1");
    let g2 = step_once(&g1, Boundary::Fixed);
    assert_grids_eq(&g2, &g0, "beacon step 2 (period)");
}

#[test]
fn pulsar_period_3() {
    // Use Simulation + place_pattern so we exercise the bundled canonical form.
    let mut sim = Simulation::new(17, 17, Boundary::Fixed).unwrap();
    let pulsar = builtin("pulsar").unwrap();
    sim.place_pattern(pulsar, 2, 2).unwrap();
    let phase0 = sim.cells().to_vec();

    sim.step();
    let phase1 = sim.cells().to_vec();
    assert_ne!(phase0, phase1, "pulsar should change at step 1");
    sim.step();
    let phase2 = sim.cells().to_vec();
    assert_ne!(phase1, phase2, "pulsar should change at step 2");
    sim.step();
    assert_eq!(sim.cells(), &phase0[..], "pulsar must return at step 3");
}

#[test]
fn pentadecathlon_period_15() {
    let mut sim = Simulation::new(20, 20, Boundary::Fixed).unwrap();
    let p = builtin("pentadecathlon").unwrap();
    sim.place_pattern(p, 5, 5).unwrap();
    let phase0 = sim.cells().to_vec();
    for s in 1..15 {
        sim.step();
        assert_ne!(
            sim.cells(),
            &phase0[..],
            "pentadecathlon must differ from phase0 at step {s}"
        );
    }
    sim.step(); // step 15
    assert_eq!(sim.cells(), &phase0[..], "pentadecathlon period 15");
}

#[test]
fn still_lifes_do_not_change() {
    for name in ["block", "beehive", "loaf"] {
        let p = builtin(name).unwrap();
        // Use a generously-padded grid so the still life sees only dead
        // neighbours regardless of boundary handling.
        let mut sim = Simulation::new(20, 20, Boundary::Fixed).unwrap();
        sim.place_pattern(p, 5, 5).unwrap();
        let before = sim.cells().to_vec();
        for _ in 0..5 {
            sim.step();
        }
        assert_eq!(sim.cells(), &before[..], "{name} must be still");
    }
}

#[test]
fn glider_exact_4_steps_displacement() {
    // Place a glider at (5,5) on a sufficiently large board so Fixed boundary
    // never interferes. After 4 generations the glider has moved by (+1, +1)
    // and the *exact* cell pattern must match a freshly-placed glider at (6,6).
    let mut a = Simulation::new(20, 20, Boundary::Fixed).unwrap();
    let glider = builtin("glider").unwrap();
    a.place_pattern(glider, 5, 5).unwrap();
    for _ in 0..4 {
        a.step();
    }
    let mut b = Simulation::new(20, 20, Boundary::Fixed).unwrap();
    b.place_pattern(glider, 6, 6).unwrap();
    assert_eq!(a.cells(), b.cells(), "glider must move (+1,+1) in 4 steps");
}

// ---------------------------------------------------------------------------
// C. Toroidal boundary — wrap at every edge & corner
// ---------------------------------------------------------------------------

/// A blinker straddling the top/bottom seam must oscillate identically to one
/// in the middle of the board.
#[test]
fn toroidal_blinker_wraps_vertically() {
    // Vertical 3-cell column at x=2, rows {h-1, 0, 1}.
    let mut g = Grid::new(5, 5).unwrap();
    let h = g.height() as i32;
    g.set(2, h - 1, Cell::Alive);
    g.set(2, 0, Cell::Alive);
    g.set(2, 1, Cell::Alive);

    let next = step_once(&g, Boundary::Toroidal);
    // After one step it becomes a horizontal 3-cell row at y=0, x in {1,2,3}.
    let expected = grid_from_rows(&[
        ".###.",
        ".....",
        ".....",
        ".....",
        ".....",
    ]);
    assert_grids_eq(&next, &expected, "toroidal blinker vertical seam");
    let next2 = step_once(&next, Boundary::Toroidal);
    assert_grids_eq(&next2, &g, "period-2 across vertical seam");
}

#[test]
fn toroidal_blinker_wraps_horizontally() {
    // Horizontal 3-cell row at y=2, x in {w-1, 0, 1}.
    let mut g = Grid::new(5, 5).unwrap();
    let w = g.width() as i32;
    g.set(w - 1, 2, Cell::Alive);
    g.set(0, 2, Cell::Alive);
    g.set(1, 2, Cell::Alive);

    let next = step_once(&g, Boundary::Toroidal);
    // After one step: vertical 3-column at x=0, y in {1,2,3}.
    let expected = grid_from_rows(&[
        ".....",
        "#....",
        "#....",
        "#....",
        ".....",
    ]);
    assert_grids_eq(&next, &expected, "toroidal blinker horizontal seam");
    let next2 = step_once(&next, Boundary::Toroidal);
    assert_grids_eq(&next2, &g, "period-2 across horizontal seam");
}

/// A glider placed at each of the 4 corners should reproduce a translated
/// glider after 4 steps with full wrap.
#[test]
fn toroidal_glider_wraps_at_each_corner() {
    let glider = builtin("glider").unwrap();
    let w = 12u32;
    let h = 12u32;
    let corners: &[(i32, i32)] = &[(0, 0), (w as i32 - 3, 0), (0, h as i32 - 3), (w as i32 - 3, h as i32 - 3)];
    for &(ox, oy) in corners {
        let mut a = Simulation::new(w, h, Boundary::Toroidal).unwrap();
        a.place_pattern(glider, ox, oy).unwrap();
        for _ in 0..4 {
            a.step();
        }
        // Reference: fresh glider at (ox+1, oy+1) modulo grid.
        let mut b = Simulation::new(w, h, Boundary::Toroidal).unwrap();
        let nx = (ox + 1).rem_euclid(w as i32);
        let ny = (oy + 1).rem_euclid(h as i32);
        b.place_pattern(glider, nx, ny).unwrap();
        assert_eq!(
            a.cells(),
            b.cells(),
            "glider wrap at corner ({ox},{oy}) failed"
        );
    }
}

// ---------------------------------------------------------------------------
// D. Fixed boundary — out-of-grid neighbours are permanently Dead
// ---------------------------------------------------------------------------

/// A glider headed into the bottom-right corner with Fixed boundary must NOT
/// wrap; it should evolve to a configuration that has lost cells compared to
/// the toroidal version.
#[test]
fn fixed_glider_does_not_wrap() {
    let glider = builtin("glider").unwrap();
    let mut fixed = Simulation::new(8, 8, Boundary::Fixed).unwrap();
    fixed.place_pattern(glider, 5, 5).unwrap();
    for _ in 0..6 {
        fixed.step();
    }
    let mut tor = Simulation::new(8, 8, Boundary::Toroidal).unwrap();
    tor.place_pattern(glider, 5, 5).unwrap();
    for _ in 0..6 {
        tor.step();
    }
    assert_ne!(fixed.cells(), tor.cells(), "fixed must diverge from toroidal");
}

/// A horizontal blinker placed flush against the top row of a Fixed-boundary
/// grid only has one row of (implicit-Dead) neighbours above it. After one
/// step, only the centre column survives (two cells), and after a second step
/// even those two die — they each have only one live neighbour. This pins
/// the "off-grid is Dead" semantics: a blinker that touches the boundary
/// CANNOT oscillate, because it would need the absent row to be alive.
#[test]
fn fixed_blinker_at_top_edge_collapses() {
    let g0 = grid_from_rows(&[
        ".###.",
        ".....",
        ".....",
        ".....",
        ".....",
    ]);
    let g1 = step_once(&g0, Boundary::Fixed);
    let expected_g1 = grid_from_rows(&[
        "..#..",
        "..#..",
        ".....",
        ".....",
        ".....",
    ]);
    assert_grids_eq(&g1, &expected_g1, "blinker at top edge step 1");
    let g2 = step_once(&g1, Boundary::Fixed);
    let expected_g2 = grid_from_rows(&[
        ".....",
        ".....",
        ".....",
        ".....",
        ".....",
    ]);
    assert_grids_eq(&g2, &expected_g2, "blinker at top edge dies at step 2");
}

/// A 2x2 block at the very corner of the grid is still a still life under
/// Fixed boundary (off-grid neighbours are Dead).
#[test]
fn fixed_block_in_corner_is_still() {
    for &(ox, oy) in &[(0, 0), (3, 0), (0, 3), (3, 3)] {
        let mut g = Grid::new(5, 5).unwrap();
        g.set(ox, oy, Cell::Alive);
        g.set(ox + 1, oy, Cell::Alive);
        g.set(ox, oy + 1, Cell::Alive);
        g.set(ox + 1, oy + 1, Cell::Alive);
        let before = g.clone();
        let after = step_once(&g, Boundary::Fixed);
        assert_grids_eq(&after, &before, "block corner ({ox},{oy})");
    }
}

// ---------------------------------------------------------------------------
// E. Degenerate shapes
// ---------------------------------------------------------------------------

#[test]
fn one_by_one_grid_always_dies() {
    let mut g = Grid::new(1, 1).unwrap();
    g.set(0, 0, Cell::Alive);
    // Fixed: 0 neighbours, dies.
    let next = step_once(&g, Boundary::Fixed);
    assert_eq!(next.get(0, 0, Boundary::Fixed), Cell::Dead);
    // Toroidal: on 1x1 every neighbour offset wraps to the SAME cell. Since
    // we explicitly skip the centre, the live cell still has 0 neighbours
    // and dies. (This also pins the "self is not its own neighbour" rule.)
    let next_t = step_once(&g, Boundary::Toroidal);
    assert_eq!(next_t.get(0, 0, Boundary::Toroidal), Cell::Dead);
}

#[test]
fn two_by_two_all_alive_is_still_under_fixed() {
    let g = grid_from_rows(&["##", "##"]);
    let next = step_once(&g, Boundary::Fixed);
    assert_grids_eq(&next, &g, "2x2 full block under Fixed");
}

#[test]
fn two_by_two_single_alive_dies_under_fixed() {
    let g = grid_from_rows(&["#.", ".."]);
    let next = step_once(&g, Boundary::Fixed);
    let expected = grid_from_rows(&["..", ".."]);
    assert_grids_eq(&next, &expected, "single cell dies under Fixed");
}

#[test]
fn one_row_grid_under_fixed_keeps_inner_three() {
    // A 1xN row has no row above or below, so the only neighbours of cell
    // (x, 0) are (x-1, 0) and (x+1, 0). Inner cells thus have 2 alive
    // neighbours and survive; the two endpoints have 1 and die. No new birth
    // is possible because birth requires neighbours from both rows.
    let g = grid_from_rows(&["#####"]);
    let next = step_once(&g, Boundary::Fixed);
    let expected = grid_from_rows(&[".###."]);
    assert_grids_eq(&next, &expected, "1xN row collapses to inner 3");
    // One more step: (1,0) and (3,0) each have a single live neighbour and
    // die; (2,0) has two and survives. Final state is a single live cell at
    // the centre, which then dies the step after.
    let next2 = step_once(&next, Boundary::Fixed);
    let expected2 = grid_from_rows(&["..#.."]);
    assert_grids_eq(&next2, &expected2, "1xN row collapses to single cell");
    let next3 = step_once(&next2, Boundary::Fixed);
    let expected3 = grid_from_rows(&["....."]);
    assert_grids_eq(&next3, &expected3, "1xN row finally dies");
}

#[test]
fn one_column_grid_under_fixed_keeps_inner_three() {
    // Same logic transposed: only vertical neighbours exist.
    let g = grid_from_rows(&["#", "#", "#", "#", "#"]);
    let next = step_once(&g, Boundary::Fixed);
    let expected = grid_from_rows(&[".", "#", "#", "#", "."]);
    assert_grids_eq(&next, &expected, "Nx1 column collapses to inner 3");
    let next2 = step_once(&next, Boundary::Fixed);
    let expected2 = grid_from_rows(&[".", ".", "#", ".", "."]);
    assert_grids_eq(&next2, &expected2, "Nx1 column collapses to single cell");
    let next3 = step_once(&next2, Boundary::Fixed);
    let expected3 = grid_from_rows(&[".", ".", ".", ".", "."]);
    assert_grids_eq(&next3, &expected3, "Nx1 column finally dies");
}

// ---------------------------------------------------------------------------
// F. Symmetry invariants
// ---------------------------------------------------------------------------

/// For an arbitrary square Toroidal board, rotating 90° and then stepping must
/// produce the same grid as stepping and then rotating 90°.
///
/// This is a strong correctness probe: any axis confusion in an optimised
/// neighbour-counting kernel (off-by-one in `y_up`/`y_down` etc.) instantly
/// breaks this property.
#[test]
fn rotate_step_commutes_under_toroidal() {
    // A small but non-trivial seed: glider + LWSS in distinct quadrants of an
    // 11x11 torus. 11 is prime so wrapping has no symmetry that could mask
    // bugs accidentally.
    let mut sim = Simulation::new(11, 11, Boundary::Toroidal).unwrap();
    sim.place_pattern(builtin("glider").unwrap(), 1, 1).unwrap();
    sim.place_pattern(builtin("blinker").unwrap(), 7, 8).unwrap();
    let mut g = Grid::new(11, 11).unwrap();
    g.cells_mut().copy_from_slice(sim.cells());

    // Path 1: rotate then step.
    let rotated = rotate_cw(&g);
    let r_then_s = step_once(&rotated, Boundary::Toroidal);
    // Path 2: step then rotate.
    let stepped = step_once(&g, Boundary::Toroidal);
    let s_then_r = rotate_cw(&stepped);

    assert_grids_eq(&r_then_s, &s_then_r, "rotate∘step == step∘rotate");
}

// ---------------------------------------------------------------------------
// G. Long-running large board: gosper-glider-gun keeps producing gliders
// ---------------------------------------------------------------------------

#[test]
fn gosper_glider_gun_emits_gliders_before_collision() {
    // The Gosper glider gun emits one glider every 30 generations into the
    // bottom-right quadrant. We use a Fixed-boundary board large enough that
    // the first two emitted gliders never reach the wall before t=90.
    //
    // We assert two robust invariants:
    //   1. The population is strictly larger at t=60 than at t=0 (>= 2 new
    //      gliders worth of cells, modulo gun flicker).
    //   2. The population at t=90 is still strictly larger than at t=0.
    // We deliberately avoid asserting strict monotonicity between samples,
    // because the gun itself flickers across the 30-step period.
    let gun = builtin("gosper-glider-gun").unwrap();
    let mut sim = Simulation::new(120, 80, Boundary::Fixed).unwrap();
    sim.place_pattern(gun, 1, 1).unwrap();
    let initial = sim.count_alive();

    for _ in 0..60 {
        sim.step();
    }
    let t60 = sim.count_alive();
    for _ in 0..30 {
        sim.step();
    }
    let t90 = sim.count_alive();

    assert!(
        t60 > initial,
        "gun must emit gliders by t=60: initial={initial} t60={t60}"
    );
    assert!(
        t90 > initial,
        "gun population must stay above initial by t=90: initial={initial} t90={t90}"
    );
}

// ---------------------------------------------------------------------------
// H. Deterministic seeded snapshot — pin the entire evolution end-to-end
// ---------------------------------------------------------------------------

/// Hash of `cells()` after a fixed sequence of operations on a seeded RNG.
/// If anyone changes the rule, the boundary semantics, the RNG selection, or
/// the cell layout, this test will detect it instantly.
///
/// The constants below were generated from the *current* implementation. If
/// you intentionally change semantics, update both hashes in one commit and
/// document why in the message.
#[test]
fn deterministic_seeded_snapshot_hashes() {
    // Toroidal, 30x20, density=0.30, seed=42, then step 100.
    let mut sim = Simulation::new(30, 20, Boundary::Toroidal).unwrap();
    sim.set_history_capacity(0); // history irrelevant to cells layout
    sim.randomize(0.30, Some(42)).unwrap();
    let h_initial = hex_hash(sim.cells());
    for _ in 0..100 {
        sim.step();
    }
    let h_after = hex_hash(sim.cells());
    assert_eq!(
        h_initial,
        "b702c430ddfd2d73f3225474bebb7bafa47ffa5932e55b319ab01269bb7601cf",
        "seeded initial hash drift (regenerate intentionally if rule changed)"
    );
    assert_eq!(
        h_after,
        "f146d2ca6e810103cf4119d57f63282bccd5f83484d27218a2340b6bf3570276",
        "seeded post-100-step hash drift"
    );
}

// ---------------------------------------------------------------------------
// Extra: bundled patterns sanity (categories cover everything)
// ---------------------------------------------------------------------------

#[test]
fn every_bundled_pattern_has_known_category() {
    for p in all_builtins() {
        let cat = p.category;
        // All four variants are valid; just exercise label() / slug() so a
        // missing arm anywhere in the codebase is flagged.
        let _ = cat.label();
        let _ = cat.slug();
        // And confirm the pattern is retrievable by name.
        assert!(builtin(p.name).is_some(), "{} must round-trip", p.name);
        // Spot check: oscillators must NOT be still after one step on a
        // generously padded board.
        if cat == PatternCategory::Oscillator {
            let mut sim = Simulation::new(p.width + 20, p.height + 20, Boundary::Fixed).unwrap();
            sim.place_pattern(p, 10, 10).unwrap();
            let before = sim.cells().to_vec();
            sim.step();
            assert_ne!(sim.cells(), &before[..], "{} should oscillate", p.name);
        }
        // Still lifes must be still.
        if cat == PatternCategory::StillLife {
            let mut sim = Simulation::new(p.width + 20, p.height + 20, Boundary::Fixed).unwrap();
            sim.place_pattern(p, 10, 10).unwrap();
            let before = sim.cells().to_vec();
            sim.step();
            assert_eq!(sim.cells(), &before[..], "{} should be still", p.name);
        }
    }
}
