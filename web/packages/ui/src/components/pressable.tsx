import type { ReactNode } from "react";
import { motion } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";
import { pressScale, springSnappy } from "../motion";

/**
 * Adds the shared press feedback (slight scale-down) to any control, e.g. a Kumo Button.
 * Wraps rather than replaces so Kumo keeps owning semantics, focus and states.
 */
export function Pressable({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <motion.span whileTap={pressScale} transition={springSnappy} className={cn("inline-flex", className)}>
      {children}
    </motion.span>
  );
}
