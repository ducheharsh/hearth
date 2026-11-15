/**
 * Original pixel-art sprites. Each sprite is a grid of palette keys ('.' = transparent).
 * Frames share dimensions so they can be swapped without layout shift.
 */

export interface Sprite {
  width: number;
  height: number;
  palette: Record<string, string>;
  rows: readonly string[];
}

export interface PetDefinition {
  name: string;
  open: Sprite;
  blink: Sprite;
}

function sprite(palette: Record<string, string>, rows: readonly string[]): Sprite {
  const width = rows[0]?.length ?? 0;
  if (import.meta.env.DEV && rows.some((r) => r.length !== width)) {
    throw new Error("sprite rows must share a width");
  }
  return { width, height: rows.length, palette, rows };
}

// ── Ember: a little hearth flame ──
const EMBER_PALETTE = {
  o: "#5b1d0f", // outline
  y: "#ffd166", // flame tip
  r: "#f97316", // body
  e: "#2a120a", // eyes
  c: "#fb7185", // cheeks
};

const EMBER_OPEN = [
  "......o......",
  ".....oyo.....",
  ".....oyyo....",
  "....oyyyo.o..",
  "...oyyyyyoyo.",
  "..oyyyyyyyyo.",
  ".oyyrrrrryyyo",
  ".oyrerrrrerro",
  ".oyrerrrrerro",
  ".oycrrrrrrcro",
  "..oyrrrrrrro.",
  "...ooooooo...",
] as const;

const EMBER_BLINK = EMBER_OPEN.map((row, i) => (i === 7 ? row.replaceAll("e", "r") : row));

// ── Miso: a small orange cat ──
const MISO_PALETTE = {
  o: "#2a2320", // outline
  f: "#f4a259", // fur
  d: "#d9822b", // fur shade
  p: "#f7a8b8", // pink (ears, nose)
  w: "#fff4e6", // muzzle
  e: "#2a2320", // eyes
};

const MISO_OPEN = [
  "..oo......oo..",
  ".opfo....ofpo.",
  ".offfoooofffo.",
  ".offffffffffo.",
  "offeffffffeffo",
  "offeffwwffeffo",
  "offffwppwffffo",
  ".offfwwwwfffo.",
  "..offffffffo..",
  ".offdffffdffo.",
  "offffffffffffo",
  ".oo.oo..oo.oo.",
] as const;

const MISO_BLINK = MISO_OPEN.map((row, i) => (i === 4 ? row.replaceAll("e", "f") : row));

export const PETS = {
  ember: {
    name: "Ember",
    open: sprite(EMBER_PALETTE, EMBER_OPEN),
    blink: sprite(EMBER_PALETTE, EMBER_BLINK),
  },
  miso: {
    name: "Miso",
    open: sprite(MISO_PALETTE, MISO_OPEN),
    blink: sprite(MISO_PALETTE, MISO_BLINK),
  },
} satisfies Record<string, PetDefinition>;

export type PetKind = keyof typeof PETS;

// ── Small effect sprites ──
export const HEART = sprite({ h: "#f43f5e", l: "#fda4af" }, [
  ".hh.hh.",
  "hlhhhhh",
  "hhhhhhh",
  ".hhhhh.",
  "..hhh..",
  "...h...",
]);

export const SWEAT = sprite({ d: "#60a5fa", l: "#dbeafe" }, [".d.", "dld", "ddd", ".d."]);

export const BOX = sprite({ o: "#6b4423", b: "#c08552", t: "#e8c39e" }, [
  "oooooo",
  "otttto",
  "obbbbo",
  "obbbbo",
  "oooooo",
]);
