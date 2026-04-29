import './styles.css';
import { Sim } from './sim.js';
import { Renderer } from './renderer.js';
import { Loop } from './loop.js';
import { wireControls, type AppState } from './controls.js';

async function main(): Promise<void> {
  const initialW = 60;
  const initialH = 40;
  const sim = await Sim.create(initialW, initialH, true);
  sim.randomize(0.3);

  const canvas = document.getElementById('board') as HTMLCanvasElement | null;
  if (!canvas) throw new Error('#board canvas not found');
  const renderer = new Renderer(canvas);

  function fitCanvas() {
    const parent = canvas!.parentElement;
    if (!parent) return;
    const rect = parent.getBoundingClientRect();
    canvas!.style.width = rect.width + 'px';
    canvas!.style.height = rect.height + 'px';
    renderer.resize(sim.width(), sim.height());
  }
  fitCanvas();
  window.addEventListener('resize', fitCanvas);

  const state: AppState = { preview: undefined };

  const generationEl = document.getElementById('generation')!;
  const aliveEl = document.getElementById('alive')!;
  const gridSizeEl = document.getElementById('grid-size')!;

  function updateStatus() {
    generationEl.textContent = String(sim.generation());
    aliveEl.textContent = String(sim.countAlive());
    gridSizeEl.textContent = `${sim.width()}×${sim.height()}`;
  }

  const loop = new Loop(
    () => sim.step(),
    () => {
      renderer.draw(sim.cellsView(), sim.width(), sim.height(), sim.strideBytes(), state.preview);
      updateStatus();
    }
  );
  loop.start();

  wireControls({ sim, renderer, loop, state, fitCanvas });
}

main().catch((e) => {
  console.error(e);
  document.body.innerHTML = `<pre style="color:#f88; padding: 16px;">Failed to initialize: ${String(e)}</pre>`;
});
