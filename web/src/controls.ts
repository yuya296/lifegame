import { Sim } from './sim.js';
import type { Renderer, PreviewState } from './renderer.js';
import type { Loop } from './loop.js';

export interface AppState {
  preview: PreviewState | undefined;
}

export interface WireOptions {
  sim: Sim;
  renderer: Renderer;
  loop: Loop;
  state: AppState;
  fitCanvas: () => void;
}

function $<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`#${id} not found`);
  return el as T;
}

export function wireControls(opts: WireOptions): void {
  const { sim, renderer, loop, state, fitCanvas } = opts;

  const playBtn = $<HTMLButtonElement>('play');
  const stepBtn = $<HTMLButtonElement>('step');
  const stepBackBtn = $<HTMLButtonElement>('step-back');
  const fpsRange = $<HTMLInputElement>('fps');
  const fpsDisplay = $<HTMLSpanElement>('fps-display');
  const widthInput = $<HTMLInputElement>('width');
  const heightInput = $<HTMLInputElement>('height');
  const resizeBtn = $<HTMLButtonElement>('resize');
  const toroidalCheck = $<HTMLInputElement>('toroidal');
  const clearBtn = $<HTMLButtonElement>('clear');
  const randomizeBtn = $<HTMLButtonElement>('randomize');
  const densityRange = $<HTMLInputElement>('density');
  const densityDisplay = $<HTMLSpanElement>('density-display');
  const patternSelect = $<HTMLSelectElement>('pattern');
  const canvas = $<HTMLCanvasElement>('board');

  // 初期表示
  widthInput.value = String(sim.width());
  heightInput.value = String(sim.height());
  toroidalCheck.checked = sim.boundaryIsToroidal();
  fpsDisplay.textContent = String(loop.fps);
  fpsRange.value = String(loop.fps);

  // パターンの一覧を埋める（カテゴリでグルーピング）
  const categoryLabels: Record<string, string> = {
    'still-life': 'Still life',
    'oscillator': 'Oscillator',
    'spaceship': 'Spaceship',
    'gun': 'Gun',
  };
  const order = ['still-life', 'oscillator', 'spaceship', 'gun'];

  const grouped = new Map<string, string[]>();
  for (const { name, category } of Sim.listPatternsByCategory()) {
    if (!grouped.has(category)) grouped.set(category, []);
    grouped.get(category)!.push(name);
  }

  for (const cat of order) {
    const names = grouped.get(cat);
    if (!names || names.length === 0) continue;
    const og = document.createElement('optgroup');
    og.label = categoryLabels[cat] ?? cat;
    for (const name of names) {
      const opt = document.createElement('option');
      opt.value = name;
      opt.textContent = name;
      og.appendChild(opt);
    }
    patternSelect.appendChild(og);
  }

  // Play/Pause
  function setPlaying(playing: boolean) {
    loop.playing = playing;
    playBtn.textContent = playing ? '⏸ Pause' : '▶ Play';
  }

  playBtn.addEventListener('click', () => setPlaying(!loop.playing));
  stepBtn.addEventListener('click', () => {
    sim.step();
  });
  stepBackBtn.addEventListener('click', () => {
    sim.stepBack();
  });

  fpsRange.addEventListener('input', () => {
    const v = Number(fpsRange.value);
    loop.fps = v;
    fpsDisplay.textContent = String(v);
  });

  densityRange.addEventListener('input', () => {
    densityDisplay.textContent = Number(densityRange.value).toFixed(2);
  });

  resizeBtn.addEventListener('click', () => {
    const w = Math.max(1, Math.min(500, Number(widthInput.value) || 1));
    const h = Math.max(1, Math.min(500, Number(heightInput.value) || 1));
    sim.resize(w, h);
    fitCanvas();
  });

  toroidalCheck.addEventListener('change', () => {
    sim.setBoundary(toroidalCheck.checked);
  });

  clearBtn.addEventListener('click', () => {
    sim.clear();
  });

  randomizeBtn.addEventListener('click', () => {
    const d = Number(densityRange.value);
    sim.randomize(d);
  });

  function clearPreview() {
    state.preview = undefined;
  }

  function setPatternMode(name: string) {
    if (!name) {
      clearPreview();
      return;
    }
    try {
      const fp = Sim.patternFootprint(name);
      state.preview = { name, ox: 0, oy: 0, footprint: fp };
    } catch (e) {
      console.warn('patternFootprint failed', e);
      clearPreview();
    }
  }

  patternSelect.addEventListener('change', () => {
    setPatternMode(patternSelect.value);
  });

  // ===== Canvas pointer 入力 =====
  let isDragging = false;
  const draggedCells = new Set<string>();

  function eventToCell(ev: PointerEvent): { x: number; y: number } | null {
    const rect = canvas.getBoundingClientRect();
    const px = ev.clientX - rect.left;
    const py = ev.clientY - rect.top;
    return renderer.pixelToCell(px, py, sim.width(), sim.height());
  }

  canvas.addEventListener('pointermove', (ev) => {
    const cell = eventToCell(ev);
    if (state.preview) {
      if (cell) {
        state.preview = { ...state.preview, ox: cell.x, oy: cell.y };
      }
      return;
    }
    if (isDragging && cell) {
      const key = `${cell.x},${cell.y}`;
      if (!draggedCells.has(key)) {
        draggedCells.add(key);
        sim.setCell(cell.x, cell.y, true);
      }
    }
  });

  canvas.addEventListener('pointerdown', (ev) => {
    const cell = eventToCell(ev);
    if (!cell) return;
    canvas.setPointerCapture(ev.pointerId);

    if (state.preview) {
      try {
        sim.placePattern(state.preview.name, cell.x, cell.y);
      } catch (e) {
        console.warn('placePattern failed', e);
      }
      return;
    }

    // toggle mode + drag
    isDragging = true;
    draggedCells.clear();
    const key = `${cell.x},${cell.y}`;
    draggedCells.add(key);
    sim.toggle(cell.x, cell.y);
  });

  function endDrag(ev: PointerEvent) {
    isDragging = false;
    draggedCells.clear();
    if (canvas.hasPointerCapture(ev.pointerId)) {
      canvas.releasePointerCapture(ev.pointerId);
    }
  }
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', endDrag);

  canvas.addEventListener('pointerleave', () => {
    if (state.preview) {
      // プレビューはマウス位置に追従するだけなので、消すかどうかは好み。
      // ここでは消さず、最後の位置に表示し続ける
    }
  });

  // ===== Keyboard =====
  window.addEventListener('keydown', (ev) => {
    // input/select にフォーカスがあれば無視
    const t = ev.target as HTMLElement | null;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'SELECT' || t.tagName === 'TEXTAREA')) {
      if (ev.key === 'Escape') {
        (t as HTMLElement).blur();
      } else {
        return;
      }
    }

    switch (ev.key) {
      case ' ':
        ev.preventDefault();
        setPlaying(!loop.playing);
        break;
      case '.':
        sim.step();
        break;
      case ',':
        sim.stepBack();
        break;
      case 'c':
      case 'C':
        sim.clear();
        break;
      case 'r':
      case 'R':
        sim.randomize(Number(densityRange.value));
        break;
      case 'Escape':
        patternSelect.value = '';
        clearPreview();
        break;
    }
  });
}
