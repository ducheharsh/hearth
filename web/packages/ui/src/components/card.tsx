import type { ComponentProps } from "react";
import { motion } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";
import { hoverLift, springSoft } from "../motion";

const base =
  "relative rounded-xl bg-kumo-base ring-1 ring-kumo-hairline shadow-[0_1px_0_0_rgb(0_0_0/0.02)]";

/** Static surface. Use for content panels. */
export function Card({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn(base, "p-4", className)} {...props} />;
}

/**
 * Surface that lifts on hover. Use for clickable/actionable tiles (app cards).
 * The lift and ring change are the shared "this is interactive" affordance.
 */
export function InteractiveCard({ className, ...props }: ComponentProps<typeof motion.div>) {
  return (
    <motion.div
      whileHover={hoverLift}
      transition={springSoft}
      className={cn(
        base,
        "p-4 transition-[box-shadow,--tw-ring-color] duration-200 hover:ring-kumo-line hover:shadow-lg focus-within:ring-kumo-line",
        className,
      )}
      {...props}
    />
  );
}
