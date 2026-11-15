import { memo } from "react";
import { cn } from "@cloudflare/kumo/utils";
import type { Sprite } from "./sprites";

interface Run {
  x: number;
  y: number;
  w: number;
  fill: string;
}

/** Merge horizontal same-color pixels into single rects (far fewer DOM nodes). */
function toRuns(s: Sprite): Run[] {
  const runs: Run[] = [];
  s.rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const key = row[x] ?? ".";
      let end = x + 1;
      while (end < row.length && row[end] === key) end++;
      const fill = s.palette[key];
      if (key !== "." && fill) runs.push({ x, y, w: end - x, fill });
      x = end;
    }
  });
  return runs;
}

const cache = new WeakMap<Sprite, Run[]>();
function runsFor(s: Sprite): Run[] {
  let r = cache.get(s);
  if (!r) {
    r = toRuns(s);
    cache.set(s, r);
  }
  return r;
}

interface PixelSpriteProps {
  sprite: Sprite;
  /** CSS pixels per sprite pixel. */
  scale?: number;
  className?: string;
}

/** Crisp pixel art as SVG. Decorative by default. */
export const PixelSprite = memo(function PixelSprite({ sprite, scale = 4, className }: PixelSpriteProps) {
  return (
    <svg
      width={sprite.width * scale}
      height={sprite.height * scale}
      viewBox={`0 0 ${sprite.width} ${sprite.height}`}
      className={cn("pixelated block", className)}
      aria-hidden="true"
    >
      {runsFor(sprite).map((r) => (
        <rect key={`${r.x}-${r.y}`} x={r.x} y={r.y} width={r.w} height={1} fill={r.fill} />
      ))}
    </svg>
  );
});
