import {
  BrainIcon,
  CubeIcon,
  FlowArrowIcon,
  HardDrivesIcon,
  HeartbeatIcon,
  ImagesIcon,
  NotebookIcon,
  ShieldCheckIcon,
  WifiHighIcon,
  type Icon,
} from "@phosphor-icons/react";
import { cn } from "@cloudflare/kumo/utils";

interface Style {
  icon: Icon;
  /** Kumo badge tokens: semantic, theme-aware tints. */
  tint: string;
}

const STYLES: Record<string, Style> = {
  security: { icon: ShieldCheckIcon, tint: "bg-kumo-badge-green text-white" },
  monitoring: { icon: HeartbeatIcon, tint: "bg-kumo-badge-red text-white" },
  network: { icon: WifiHighIcon, tint: "bg-kumo-badge-blue text-white" },
  productivity: { icon: NotebookIcon, tint: "bg-kumo-badge-orange text-white" },
  automation: { icon: FlowArrowIcon, tint: "bg-kumo-badge-purple text-white" },
  media: { icon: ImagesIcon, tint: "bg-kumo-badge-teal text-white" },
  ai: { icon: BrainIcon, tint: "bg-kumo-badge-purple text-white" },
  storage: { icon: HardDrivesIcon, tint: "bg-kumo-badge-blue text-white" },
};
const FALLBACK: Style = { icon: CubeIcon, tint: "bg-kumo-badge-neutral text-white" };

/** Rounded tile with the category's icon. Decorative: the name is always shown next to it. */
export function CategoryIcon({ category, className }: { category: string; className?: string }) {
  const { icon: I, tint } = STYLES[category] ?? FALLBACK;
  return (
    <span
      aria-hidden="true"
      className={cn("inline-flex size-10 shrink-0 items-center justify-center rounded-lg shadow-sm", tint, className)}
    >
      <I size={20} weight="duotone" />
    </span>
  );
}
