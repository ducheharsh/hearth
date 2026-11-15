import { motion } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";
import { springSoft } from "../motion";

/** Pick a tone from a 0–100 fill percentage. Shared so every meter agrees. */
export function toneFor(percent: number): "brand" | "warning" | "danger" {
  if (percent >= 90) return "danger";
  if (percent >= 75) return "warning";
  return "brand";
}

const FILL = {
  brand: "bg-kumo-brand",
  warning: "bg-kumo-warning",
  danger: "bg-kumo-danger",
} as const;

interface UsageBarProps {
  value: number;
  max: number;
  label: string;
  /** Right-aligned detail text, e.g. "3.1 GB of 8 GB". */
  detail?: string;
  /** Optional second marker, e.g. projected usage after an install. */
  projected?: number;
  className?: string;
}

/** Thin animated bar; fill is a GPU transform (scaleX), never width. */
export function UsageBar({ value, max, label, detail, projected, className }: UsageBarProps) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  const projPct = projected != null && max > 0 ? Math.min(100, (projected / max) * 100) : null;
  const tone = toneFor(projPct ?? pct);
  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      <div className="flex items-baseline justify-between gap-3 text-xs">
        <span className="text-kumo-subtle">{label}</span>
        {detail ? <span className="tabular-nums text-kumo-strong">{detail}</span> : null}
      </div>
      <div
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={Math.round(value)}
        aria-valuetext={detail}
        className="relative h-1.5 overflow-hidden rounded-full bg-kumo-fill"
      >
        {projPct != null ? (
          <motion.div
            className={cn("absolute inset-0 origin-left rounded-full opacity-35", FILL[tone])}
            initial={false}
            animate={{ scaleX: projPct / 100 }}
            transition={springSoft}
          />
        ) : null}
        <motion.div
          className={cn("absolute inset-0 origin-left rounded-full", FILL[tone])}
          initial={{ scaleX: 0 }}
          animate={{ scaleX: pct / 100 }}
          transition={springSoft}
        />
      </div>
    </div>
  );
}
