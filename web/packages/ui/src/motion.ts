/**
 * Motion tokens. Every animated component uses these so the whole UI moves the same way.
 * Only transform/opacity are animated (compositor-friendly).
 */
import type { Transition, Variants } from "motion/react";

/** Snappy UI feedback (buttons, toggles, chips). */
export const springSnappy: Transition = { type: "spring", stiffness: 500, damping: 34, mass: 0.7 };
/** Layout shifts, cards entering/leaving, dialogs. */
export const springSoft: Transition = { type: "spring", stiffness: 260, damping: 28 };
/** Playful bounces (pets, success). */
export const springBouncy: Transition = { type: "spring", stiffness: 600, damping: 14 };

export const easeOutSoft = [0.22, 1, 0.36, 1] as const;

export const pressScale = { scale: 0.97 } as const;
export const hoverLift = { y: -2 } as const;

/** Fade + rise for lists/sections; pair with `custom={index}` for a stagger. */
export const riseIn: Variants = {
  hidden: { opacity: 0, y: 8 },
  show: (i: number = 0) => ({
    opacity: 1,
    y: 0,
    transition: { ...springSoft, delay: Math.min(i * 0.035, 0.3) },
  }),
  exit: { opacity: 0, y: -4, scale: 0.98, transition: { duration: 0.15 } },
};
