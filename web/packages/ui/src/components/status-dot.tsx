import { cn } from "@cloudflare/kumo/utils";

type Tone = "success" | "warning" | "danger" | "neutral";

const COLOR: Record<Tone, string> = {
  success: "bg-kumo-success",
  warning: "bg-kumo-warning",
  danger: "bg-kumo-danger",
  neutral: "bg-kumo-inactive",
};

interface StatusDotProps {
  tone: Tone;
  /** Pulsing halo: the thing is live/active right now. */
  live?: boolean;
  label: string;
  className?: string;
}

export function StatusDot({ tone, live, label, className }: StatusDotProps) {
  return (
    <span className={cn("relative inline-flex size-2 shrink-0", className)} role="img" aria-label={label}>
      {live ? (
        <span className={cn("absolute inset-0 rounded-full animate-ping-soft", COLOR[tone])} aria-hidden="true" />
      ) : null}
      <span className={cn("relative size-2 rounded-full", COLOR[tone])} aria-hidden="true" />
    </span>
  );
}
