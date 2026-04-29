//! Conway's Game of Life — CLI renderer.
//!
//! Renders the simulation to the terminal using a few ANSI escapes
//! (no `crossterm`). Ctrl-C is captured via the `ctrlc` crate so we exit
//! cleanly between frames.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use lifegame_core::{all_builtins, builtin_or_err, Boundary, PatternCategory, Simulation};

/// ANSI: clear screen + move cursor home.
const ANSI_CLEAR: &str = "\x1b[2J\x1b[H";

/// CLI boundary kind, mapped onto `lifegame_core::Boundary`.
#[derive(Copy, Clone, Debug, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum BoundaryArg {
    Toroidal,
    Fixed,
}

impl From<BoundaryArg> for Boundary {
    fn from(b: BoundaryArg) -> Self {
        match b {
            BoundaryArg::Toroidal => Boundary::Toroidal,
            BoundaryArg::Fixed => Boundary::Fixed,
        }
    }
}

impl BoundaryArg {
    fn label(self) -> &'static str {
        match self {
            BoundaryArg::Toroidal => "toroidal",
            BoundaryArg::Fixed => "fixed",
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "lifegame",
    about = "Conway's Game of Life — terminal renderer",
    version
)]
struct Cli {
    /// Grid width
    #[arg(long, default_value_t = 40)]
    width: u32,

    /// Grid height
    #[arg(long, default_value_t = 20)]
    height: u32,

    /// Number of generations to run (default: unlimited until Ctrl-C)
    #[arg(long)]
    steps: Option<u64>,

    /// Starting pattern: `random` or a builtin pattern name
    #[arg(long, default_value = "random")]
    pattern: String,

    /// Density when `--pattern random`
    #[arg(long, default_value_t = 0.3)]
    density: f32,

    /// Grid boundary
    #[arg(long, value_enum, default_value_t = BoundaryArg::Toroidal)]
    boundary: BoundaryArg,

    /// Frames per second
    #[arg(long, default_value_t = 8.0)]
    fps: f32,

    /// RNG seed (only used with `--pattern random`)
    #[arg(long)]
    seed: Option<u64>,

    /// Render one frame and exit (CI / snapshot friendly)
    #[arg(long)]
    once: bool,

    /// Do not clear the terminal between frames
    #[arg(long)]
    no_clear: bool,

    /// List builtin pattern names and exit
    #[arg(long)]
    list_patterns: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_patterns {
        print_pattern_list();
        return Ok(());
    }

    let mut sim = Simulation::new(cli.width, cli.height, cli.boundary.into())
        .context("failed to create simulation")?;

    if cli.pattern == "random" {
        sim.randomize(cli.density, cli.seed)
            .context("failed to randomize initial state")?;
    } else {
        let pat = builtin_or_err(&cli.pattern).with_context(|| {
            format!(
                "unknown pattern '{}'. Run with --list-patterns to see available names.",
                cli.pattern
            )
        })?;
        // Centred placement. The grid dimensions are bounded by `i32::MAX`
        // (enforced by `Grid::new`), so these casts are safe.
        let ox = center_offset(cli.width, pat.width);
        let oy = center_offset(cli.height, pat.height);
        sim.place_pattern(pat, ox, oy).with_context(|| {
            format!(
                "failed to place pattern '{}' on {}x{} grid",
                pat.name, cli.width, cli.height
            )
        })?;
    }

    if cli.fps <= 0.0 || !cli.fps.is_finite() {
        return Err(anyhow!("--fps must be a positive finite number"));
    }

    // Ctrl-C handler: flips a shared flag the render loop polls.
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        // Tolerate "handler already set" errors so repeated invocations within
        // a single process (e.g. tests running in parallel) don't panic, but
        // still warn the user so a missing handler isn't silently swallowed.
        if let Err(e) = ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        }) {
            eprintln!("warning: failed to install Ctrl-C handler: {e}");
        }
    }

    run_loop(&mut sim, &cli, stop)
}

/// Print all builtin patterns grouped by category, in a stable order.
fn print_pattern_list() {
    let order = [
        PatternCategory::StillLife,
        PatternCategory::Oscillator,
        PatternCategory::Spaceship,
        PatternCategory::Gun,
    ];
    let pats = all_builtins();
    for cat in order {
        let in_cat: Vec<&str> = pats
            .iter()
            .filter(|p| p.category == cat)
            .map(|p| p.name)
            .collect();
        if in_cat.is_empty() {
            continue;
        }
        println!("{}:", cat.label());
        for name in in_cat {
            println!("  {name}");
        }
    }
}

/// Compute the centred placement offset along one axis using floor division
/// so that patterns larger than the grid are still centred symmetrically
/// (e.g. `grid=2, pattern=3` yields `-1`, not `0`).
fn center_offset(grid_dim: u32, pattern_dim: u32) -> i32 {
    let g = grid_dim as i32;
    let p = pattern_dim as i32;
    (g - p).div_euclid(2)
}

fn run_loop(sim: &mut Simulation, cli: &Cli, stop: Arc<AtomicBool>) -> Result<()> {
    let frame_delay = Duration::from_secs_f32(1.0 / cli.fps);
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Render the initial frame (gen 0).
    render_frame(sim, cli, &mut out)?;

    if cli.once {
        return Ok(());
    }

    // `--steps N` advances the simulation N times (gen 0 + N stepped frames).
    let max_steps = cli.steps.unwrap_or(u64::MAX);
    let mut steps_done: u64 = 0;
    while steps_done < max_steps {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(frame_delay);
        if stop.load(Ordering::SeqCst) {
            break;
        }

        sim.step();
        steps_done += 1;
        render_frame(sim, cli, &mut out)?;
    }

    Ok(())
}

fn render_frame<W: Write>(sim: &Simulation, cli: &Cli, out: &mut W) -> io::Result<()> {
    if !cli.no_clear && !cli.once {
        out.write_all(ANSI_CLEAR.as_bytes())?;
    }

    // Header.
    writeln!(
        out,
        "Gen: {} | Alive: {} | {}x{} | {} | {:.1} fps",
        sim.generation(),
        sim.count_alive(),
        sim.width(),
        sim.height(),
        cli.boundary.label(),
        cli.fps,
    )?;

    // Board.
    render_board(sim, out)?;
    out.flush()
}

fn render_board<W: Write>(sim: &Simulation, out: &mut W) -> io::Result<()> {
    let w = sim.width() as usize;
    let h = sim.height() as usize;
    // Bit-packed layout: cell (x, y) is bit `x & 63` of word
    // `bits[y * stride + (x >> 6)]`.
    let bits = sim.bits();
    let stride = (w + 63) / 64;
    let mut line = String::with_capacity(w * 3);
    for y in 0..h {
        line.clear();
        let row_base = y * stride;
        for x in 0..w {
            let word = bits[row_base + (x >> 6)];
            let alive = (word >> (x & 63)) & 1 != 0;
            line.push(if alive { '█' } else { '·' });
        }
        writeln!(out, "{line}")?;
    }
    Ok(())
}
