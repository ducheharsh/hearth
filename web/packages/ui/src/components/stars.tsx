import { useState } from "react";
import { motion } from "motion/react";
import { StarIcon } from "@phosphor-icons/react";
import { cn } from "@cloudflare/kumo/utils";
import { springBouncy } from "../motion";

interface StarsProps {
  value: number | null | undefined;
  /** When set, the stars are an interactive rating control. */
  onChange?: (stars: number | null) => void;
  size?: number;
  label?: string;
  className?: string;
}

/** 1–5 star display, or an accessible radio-style rating input. Click the same star to clear. */
export function Stars({ value, onChange, size = 16, label = "Rating", className }: StarsProps) {
  const [hover, setHover] = useState<number | null>(null);
  const shown = hover ?? value ?? 0;
  if (!onChange) {
    return (
      <span className={cn("inline-flex items-center gap-0.5 text-kumo-warning", className)} role="img" aria-label={`${value ?? 0} of 5 stars`}>
        {[1, 2, 3, 4, 5].map((n) => (
          <StarIcon key={n} size={size} weight={n <= shown ? "fill" : "regular"} aria-hidden="true" />
        ))}
      </span>
    );
  }
  return (
    <span role="radiogroup" aria-label={label} className={cn("inline-flex items-center gap-0.5", className)} onMouseLeave={() => setHover(null)}>
      {[1, 2, 3, 4, 5].map((n) => (
        <motion.button
          key={n}
          type="button"
          role="radio"
          aria-checked={value === n}
          aria-label={`${n} star${n > 1 ? "s" : ""}`}
          whileTap={{ scale: 0.8 }}
          whileHover={{ scale: 1.15 }}
          transition={springBouncy}
          onMouseEnter={() => setHover(n)}
          onFocus={() => setHover(n)}
          onBlur={() => setHover(null)}
          onClick={() => onChange(value === n ? null : n)}
          className={cn("rounded p-0.5", n <= shown ? "text-kumo-warning" : "text-kumo-inactive")}
        >
          <StarIcon size={size} weight={n <= shown ? "fill" : "regular"} aria-hidden="true" />
        </motion.button>
      ))}
    </span>
  );
}
