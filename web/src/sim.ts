import init, { WasmSimulation } from './wasm/lifegame_wasm.js';

let memory: WebAssembly.Memory | undefined;

export async function initWasm(): Promise<WebAssembly.Memory> {
  if (memory) return memory;
  const out = await init();
  memory = out.memory;
  return memory;
}

export class Sim {
  constructor(public inner: WasmSimulation) {}

  static async create(width: number, height: number, toroidal: boolean): Promise<Sim> {
    await initWasm();
    return new Sim(new WasmSimulation(width, height, toroidal));
  }

  /**
   * 毎フレーム再取得すること（resize 等で memory が detach されうるため）。
   *
   * 返ってくる Uint8Array は **bit-packed** layout:
   *   - 1 bit = 1 cell (LSB が左端のセル)
   *   - 1 row = `strideBytes()` バイト（常に 8 の倍数）
   *   - cell (x, y) の生死: `(view[y * strideBytes + (x >> 3)] >> (x & 7)) & 1`
   */
  cellsView(): Uint8Array {
    const ptr = this.inner.cellsPtr();
    const len = this.inner.cellsLen();
    // memory.buffer は resize で更新される可能性がある
    return new Uint8Array(memory!.buffer, ptr, len);
  }

  /** 1行あたりのバイト数（bit-packed layout）。常に 8 の倍数。 */
  strideBytes(): number { return this.inner.strideBytes(); }

  width(): number { return this.inner.width(); }
  height(): number { return this.inner.height(); }
  generation(): number { return this.inner.generation(); }
  countAlive(): number { return this.inner.countAlive(); }
  step(): void { this.inner.step(); }
  stepBack(): boolean { return this.inner.stepBack(); }
  clear(): void { this.inner.clear(); }
  toggle(x: number, y: number): void { this.inner.toggleCell(x, y); }
  setCell(x: number, y: number, alive: boolean): void { this.inner.setCell(x, y, alive); }
  randomize(density: number, seed?: bigint): void {
    if (seed === undefined) {
      this.inner.randomize(density);
    } else {
      this.inner.randomize(density, seed);
    }
  }
  resize(width: number, height: number): void { this.inner.resize(width, height); }
  setBoundary(toroidal: boolean): void { this.inner.setBoundary(toroidal); }
  boundaryIsToroidal(): boolean { return this.inner.boundaryIsToroidal(); }
  placePattern(name: string, ox: number, oy: number): void { this.inner.placePattern(name, ox, oy); }

  static patternFootprint(name: string): Uint32Array { return WasmSimulation.patternFootprint(name); }
  static listPatterns(): string[] { return WasmSimulation.listPatterns(); }
  static listPatternsByCategory(): { name: string; category: string }[] {
    const flat = WasmSimulation.listPatternsWithCategory();
    const out: { name: string; category: string }[] = [];
    for (let i = 0; i < flat.length; i += 2) {
      out.push({ name: flat[i], category: flat[i + 1] });
    }
    return out;
  }
}
