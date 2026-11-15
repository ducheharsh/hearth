import type { ReactNode } from "react";
import type { Icon } from "@phosphor-icons/react";
import { Card } from "./card";

interface StatTileProps {
  label: string;
  icon: Icon;
  /** Main figure. Usually an <AnimatedNumber/>. */
  value: ReactNode;
  /** Small text under the value. */
  hint?: ReactNode;
  /** Visualization slot: <Sparkline/>, <UsageBar/>… */
  children?: ReactNode;
}

export function StatTile({ label, icon: I, value, hint, children }: StatTileProps) {
  return (
    <Card className="flex min-w-0 flex-col gap-3">
      <div className="flex items-center gap-2 text-xs font-medium text-kumo-subtle">
        <I size={14} aria-hidden="true" />
        <span className="truncate">{label}</span>
      </div>
      <div className="min-w-0">
        <div className="text-2xl font-semibold tracking-tight text-kumo-default">{value}</div>
        {hint ? <div className="truncate text-xs text-kumo-subtle">{hint}</div> : null}
      </div>
      {children}
    </Card>
  );
}
