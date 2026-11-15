/**
 * Renders Ember (our original pixel sprite) into a spritesheet that the agent-pet widget
 * (github.com/gibbon/agent-pet, Apache-2.0) can animate. Generated at runtime on a canvas:
 * no image assets, no network requests.
 *
 * Rows follow agent-pet's standard ids so its built-in state mapping works:
 * idle · running-right · running-left · waving · jumping · failed · waiting · running · review
 */
import { HEART, PETS, SWEAT, type Sprite } from "./sprites";

export interface AtlasRow {
  index: number;
  id: string;
  frames: number;
  fps: number;
}

export interface EmberAtlas {
  url: string;
  atlas: { cols: number; rows: number; rowsDef: AtlasRow[] };
}

const CELL_W = 96;
const CELL_H = 104;
const PX = 6; // screen pixels per sprite pixel
const COLS = 8;

const ROWS: AtlasRow[] = [
  { index: 0, id: "idle", frames: 6, fps: 6 },
  { index: 1, id: "running-right", frames: 8, fps: 10 },
  { index: 2, id: "running-left", frames: 8, fps: 10 },
  { index: 3, id: "waving", frames: 4, fps: 6 },
  { index: 4, id: "jumping", frames: 5, fps: 7 },
  { index: 5, id: "failed", frames: 8, fps: 7 },
  { index: 6, id: "waiting", frames: 6, fps: 5 },
  { index: 7, id: "running", frames: 6, fps: 9 },
  { index: 8, id: "review", frames: 6, fps: 5 },
];

interface Pose {
  dx?: number;
  dy?: number;
  rot?: number;
  sx?: number;
  sy?: number;
  blink?: boolean;
  flip?: boolean;
  /** Flicker the flame tip (shift the top rows). */
  flicker?: number;
  overlay?: "sweat" | "heart" | "dots" | "box";
  dots?: number;
}

function drawSprite(ctx: CanvasRenderingContext2D, s: Sprite, scale: number, ox: number, oy: number, flicker = 0) {
  s.rows.forEach((row, y) => {
    const shift = y < 4 ? flicker : 0;
    for (let x = 0; x < row.length; x++) {
      const key = row[x] ?? ".";
      const fill = s.palette[key];
      if (key === "." || !fill) continue;
      ctx.fillStyle = fill;
      ctx.fillRect(ox + (x + shift) * scale, oy + y * scale, scale, scale);
    }
  });
}

function drawFrame(ctx: CanvasRenderingContext2D, col: number, row: number, p: Pose) {
  const ember = PETS.ember;
  const sprite = p.blink ? ember.blink : ember.open;
  const w = sprite.width * PX;
  const h = sprite.height * PX;
  const cx = col * CELL_W + CELL_W / 2;
  const groundY = row * CELL_H + CELL_H - 10;

  // Soft ground shadow (shrinks when airborne).
  const lift = Math.max(0, -(p.dy ?? 0));
  ctx.fillStyle = "rgba(0,0,0,0.18)";
  ctx.beginPath();
  ctx.ellipse(cx, groundY + 4, Math.max(10, w / 2.4 - lift / 2), 4, 0, 0, Math.PI * 2);
  ctx.fill();

  ctx.save();
  ctx.translate(cx + (p.dx ?? 0), groundY + (p.dy ?? 0));
  ctx.rotate(((p.rot ?? 0) * Math.PI) / 180);
  ctx.scale((p.flip ? -1 : 1) * (p.sx ?? 1), p.sy ?? 1);
  drawSprite(ctx, sprite, PX, -w / 2, -h, p.flicker ?? 0);
  ctx.restore();

  const top = groundY + (p.dy ?? 0) - h;
  if (p.overlay === "sweat") drawSprite(ctx, SWEAT, 4, cx + w / 2 - 8, top + 10);
  if (p.overlay === "heart") drawSprite(ctx, HEART, 4, cx - 14, top - 26);
  if (p.overlay === "dots") {
    ctx.fillStyle = "#a8a29e";
    for (let i = 0; i < (p.dots ?? 0); i++) ctx.fillRect(cx - 12 + i * 10, top - 14, 6, 6);
  }
  if (p.overlay === "box") {
    ctx.fillStyle = "#6b4423";
    ctx.fillRect(cx - 16, top - 22, 32, 22);
    ctx.fillStyle = "#c08552";
    ctx.fillRect(cx - 12, top - 18, 24, 14);
  }
}

const wave = (i: number, n: number, amp: number) => Math.sin((i / n) * Math.PI * 2) * amp;

function poses(id: string, n: number): Pose[] {
  return Array.from({ length: n }, (_, i): Pose => {
    const flicker = i % 3 === 0 ? 1 : i % 3 === 1 ? 0 : -1;
    switch (id) {
      case "idle":
        return { dy: wave(i, n, 3), blink: i === n - 1, flicker };
      case "running-right":
      case "running-left":
        return { dx: wave(i, n, 4), dy: -Math.abs(wave(i, n / 2, 8)), rot: id === "running-right" ? 6 : -6, flicker, flip: id === "running-left" };
      case "waving":
        return { rot: i % 2 === 0 ? -10 : 10, dy: -4, flicker, overlay: "heart" };
      case "jumping": {
        const arc = [0, -18, -30, -18, 0][i] ?? 0;
        return { dy: arc, sx: i === 0 || i === 4 ? 1.12 : 0.94, sy: i === 0 || i === 4 ? 0.88 : 1.06, flicker };
      }
      case "failed":
        return { dy: 4, sy: 0.9 + (i % 2) * 0.03, blink: true, dx: i % 2 === 0 ? -1 : 1, overlay: "sweat" };
      case "waiting":
        return { sy: 1 + wave(i, n, 0.04), blink: i === 3, flicker };
      case "running":
        return { dy: -Math.abs(wave(i, n / 2, 6)), flicker, overlay: "box" };
      case "review":
        return { rot: wave(i, n, 6), flicker, overlay: "dots", dots: (i % 3) + 1 };
      default:
        return {};
    }
  });
}

let cached: EmberAtlas | null = null;

/** Build (once) the Ember spritesheet as a PNG data URL plus its atlas layout. */
export function emberAtlas(): EmberAtlas {
  if (cached) return cached;
  const canvas = document.createElement("canvas");
  canvas.width = COLS * CELL_W;
  canvas.height = ROWS.length * CELL_H;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas unavailable");
  ctx.imageSmoothingEnabled = false;
  for (const row of ROWS) {
    poses(row.id, row.frames).forEach((p, col) => drawFrame(ctx, col, row.index, p));
  }
  cached = { url: canvas.toDataURL("image/png"), atlas: { cols: COLS, rows: ROWS.length, rowsDef: ROWS } };
  return cached;
}
