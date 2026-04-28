export class Loop {
  private rafId: number | null = null;
  private accum = 0;
  private last = 0;
  public playing = false;
  public fps = 10;
  public maxStepsPerFrame = 4;

  constructor(private onTick: () => void, private onRender: () => void) {}

  start(): void {
    if (this.rafId !== null) return;
    this.last = performance.now();
    const tick = (now: number) => {
      const dt = now - this.last;
      this.last = now;
      if (this.playing) {
        this.accum += dt;
        const step = 1000 / this.fps;
        let steps = 0;
        while (this.accum >= step && steps < this.maxStepsPerFrame) {
          this.onTick();
          this.accum -= step;
          steps++;
        }
        if (this.accum > step * 4) this.accum = 0;
      }
      this.onRender();
      this.rafId = requestAnimationFrame(tick);
    };
    this.rafId = requestAnimationFrame(tick);
  }

  stop(): void {
    if (this.rafId !== null) cancelAnimationFrame(this.rafId);
    this.rafId = null;
  }
}
