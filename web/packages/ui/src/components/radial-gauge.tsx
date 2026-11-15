import type { ReactNode } from "react";
import { motion } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";
import { springSoft } from "../motion";
import { toneFor } from "./usage-bar";

const STROKE = { brand: "stroke-kumo-brand", warning: "stroke-kumo-warning", danger: "stroke-kumo-danger" } as const;

interface RadialGaugeProps {
  /** 0–100 */
  percent: number;
  label: string;
  /** Centre figure (defaults to the percentage). */
  value?: ReactNode;
  detail?: ReactNode;
  className?: string;
}

/** 270° arc gauge; colour follows the shared usage thresholds. */
export function RadialGauge({ percent, label, value, detail, className }: RadialGaugeProps) {
  const p = Math.max(0, Math.min(100, percent));
  const tone = toneFor(p);
  const r = 42;
  const circ = 2 * Math.PI * r;
  const arc = circ * 0.75;
  return (
    <div className={cn("flex flex-col items-center gap-1", className)}>
      <div className="relative size-24 sm:size-32" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(p)}>
        <svg viewBox="0 0 100 100" className="size-full -rotate-[225deg]" aria-hidden="true">
          <circle cx="50" cy="50" r={r} fill="none" strokeWidth="8" strokeLinecap="round" className="stroke-kumo-fill" strokeDasharray={`${arc} ${circ}`} />
          <motion.circle
            cx="50"
            cy="50"
            r={r}
            fill="none"
            strokeWidth="8"
            strokeLinecap="round"
            className={STROKE[tone]}
            strokeDasharray={`${arc} ${circ}`}
            initial={{ strokeDashoffset: arc }}
            animate={{ strokeDashoffset: arc * (1 - p / 100) }}
            transition={springSoft}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-lg font-semibold tabular-nums tracking-tight sm:text-2xl">{value ?? `${Math.round(p)}%`}</span>
          <span className="text-xs text-kumo-subtle">{label}</span>
        </div>
      </div>
      {detail ? <div className="max-w-40 text-center text-xs text-kumo-subtle text-pretty">{detail}</div> : null}
    </div>
  );
}
