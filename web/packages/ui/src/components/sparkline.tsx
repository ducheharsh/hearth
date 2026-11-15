import { useId, useMemo } from "react";
import { motion } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";
import { easeOutSoft } from "../motion";

type Tone = "brand" | "success" | "warning" | "danger";

const TONE: Record<Tone, string> = {
  brand: "text-kumo-brand",
  success: "text-kumo-success",
  warning: "text-kumo-warning",
  danger: "text-kumo-danger",
};

interface SparklineProps {
  values: readonly number[];
  /** Upper bound of the y axis. Defaults to the series max. */
  max?: number;
  tone?: Tone;
  className?: string;
  /** Accessible summary, e.g. "CPU over the last hour". */
  label: string;
}

const W = 100;
const H = 32;

/** Tiny area chart. Draws itself in once on mount, then morphs smoothly. */
export function Sparkline({ values, max, tone = "brand", className, label }: SparklineProps) {
  const gradientId = useId();
  const clipId = useId();
  const { line, area } = useMemo(() => {
    if (values.length < 2) return { line: "", area: "" };
    const top = max ?? Math.max(...values, 1);
    const step = W / (values.length - 1);
    const pts = values.map((v, i) => {
      const y = H - (Math.min(Math.max(v, 0), top) / top) * (H - 2) - 1;
      return `${(i * step).toFixed(1)},${y.toFixed(1)}`;
    });
    const l = `M${pts.join("L")}`;
    return { line: l, area: `${l}L${W},${H}L0,${H}Z` };
  }, [values, max]);

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={label}
      className={cn("h-10 w-full overflow-visible", TONE[tone], className)}
    >
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.22" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
        {/* Reveal left→right once on mount (a dash-based draw-in breaks with non-scaling strokes). */}
        <clipPath id={clipId}>
          <motion.rect x={0} y={-2} height={H + 4} initial={{ width: 0 }} animate={{ width: W }} transition={{ duration: 0.9, ease: easeOutSoft }} />
        </clipPath>
      </defs>
      {line ? (
        <g clipPath={`url(#${clipId})`}>
          <motion.path d={area} fill={`url(#${gradientId})`} animate={{ d: area }} transition={{ duration: 0.6, ease: easeOutSoft }} />
          <motion.path
            d={line}
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            vectorEffect="non-scaling-stroke"
            strokeLinejoin="round"
            animate={{ d: line }}
            transition={{ duration: 0.6, ease: easeOutSoft }}
          />
        </g>
      ) : null}
    </svg>
  );
}
