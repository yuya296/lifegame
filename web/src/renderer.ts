export interface RenderOptions {
  cssColor: string;
  aliveColor: string;
  gridLineColor: string;
  previewColor: string;
}

const DEFAULT_OPTIONS: RenderOptions = {
  cssColor: '#0f1419',
  aliveColor: '#7fdbca',
  gridLineColor: 'rgba(255, 255, 255, 0.06)',
  previewColor: 'rgba(255, 200, 100, 0.5)',
};

export interface PreviewState {
  name: string;
  ox: number;
  oy: number;
  footprint: Uint32Array;
}

export class Renderer {
  private ctx: CanvasRenderingContext2D;
  private cellSize: number = 1;
  private offsetX: number = 0;
  private offsetY: number = 0;

  constructor(public canvas: HTMLCanvasElement, public options: RenderOptions = DEFAULT_OPTIONS) {
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('2D context not available');
    this.ctx = ctx;
  }

  resize(gridW: number, gridH: number): void {
    const dpr = window.devicePixelRatio || 1;
    const cssW = this.canvas.clientWidth;
    const cssH = this.canvas.clientHeight;
    this.canvas.width = Math.max(1, Math.floor(cssW * dpr));
    this.canvas.height = Math.max(1, Math.floor(cssH * dpr));
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.cellSize = Math.max(1, Math.floor(Math.min(cssW / gridW, cssH / gridH)));
    this.offsetX = Math.floor((cssW - this.cellSize * gridW) / 2);
    this.offsetY = Math.floor((cssH - this.cellSize * gridH) / 2);
  }

  pixelToCell(px: number, py: number, gridW: number, gridH: number): { x: number; y: number } | null {
    const x = Math.floor((px - this.offsetX) / this.cellSize);
    const y = Math.floor((py - this.offsetY) / this.cellSize);
    if (x < 0 || y < 0 || x >= gridW || y >= gridH) return null;
    return { x, y };
  }

  draw(cells: Uint8Array, gridW: number, gridH: number, preview?: PreviewState): void {
    const { ctx, cellSize, offsetX, offsetY, options } = this;
    const cssW = this.canvas.clientWidth;
    const cssH = this.canvas.clientHeight;

    ctx.fillStyle = options.cssColor;
    ctx.fillRect(0, 0, cssW, cssH);

    ctx.fillStyle = options.aliveColor;
    for (let y = 0; y < gridH; y++) {
      const row = y * gridW;
      for (let x = 0; x < gridW; x++) {
        if (cells[row + x]) {
          ctx.fillRect(offsetX + x * cellSize, offsetY + y * cellSize, cellSize, cellSize);
        }
      }
    }

    if (preview) {
      ctx.fillStyle = options.previewColor;
      const fp = preview.footprint;
      for (let i = 0; i + 1 < fp.length; i += 2) {
        const x = preview.ox + fp[i];
        const y = preview.oy + fp[i + 1];
        if (x >= 0 && y >= 0 && x < gridW && y < gridH) {
          ctx.fillRect(offsetX + x * cellSize, offsetY + y * cellSize, cellSize, cellSize);
        }
      }
    }

    if (cellSize >= 8) {
      ctx.strokeStyle = options.gridLineColor;
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let x = 0; x <= gridW; x++) {
        const px = offsetX + x * cellSize + 0.5;
        ctx.moveTo(px, offsetY);
        ctx.lineTo(px, offsetY + gridH * cellSize);
      }
      for (let y = 0; y <= gridH; y++) {
        const py = offsetY + y * cellSize + 0.5;
        ctx.moveTo(offsetX, py);
        ctx.lineTo(offsetX + gridW * cellSize, py);
      }
      ctx.stroke();
    }
  }
}
