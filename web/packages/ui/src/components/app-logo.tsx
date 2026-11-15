import { useState } from "react";
import { cn } from "@cloudflare/kumo/utils";
import { CategoryIcon } from "./category-icon";

interface AppLogoProps {
  /** Logo URL (served by hearthd); falls back to the category tile. */
  src?: string | null;
  name: string;
  category: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}

const SIZE = { sm: "size-8 rounded-lg p-1", md: "size-10 rounded-lg p-1.5", lg: "size-16 rounded-2xl p-2.5" } as const;
const PX = { sm: 32, md: 40, lg: 64 } as const;

/** The app's real logo on a neutral tile; category icon if it's missing or fails. */
export function AppLogo({ src, name, category, size = "md", className }: AppLogoProps) {
  const [failed, setFailed] = useState(false);
  if (!src || failed) {
    return <CategoryIcon category={category} className={cn(size === "lg" && "size-16 rounded-2xl", size === "sm" && "size-8", className)} />;
  }
  return (
    <span className={cn("inline-flex shrink-0 items-center justify-center bg-white shadow-sm ring-1 ring-kumo-hairline", SIZE[size], className)}>
      <img
        src={src}
        alt=""
        aria-hidden="true"
        width={PX[size]}
        height={PX[size]}
        loading="lazy"
        decoding="async"
        onError={() => setFailed(true)}
        className="size-full object-contain"
        data-name={name}
      />
    </span>
  );
}
