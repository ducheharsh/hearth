import { useEffect } from "react";
import { motion, useReducedMotion, useSpring, useTransform } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";

const defaultFormat = (n: number) => Math.round(n).toString();

interface AnimatedNumberProps {
  value: number;
  /** Keep this stable (module-level), it runs every frame. */
  format?: (n: number) => string;
  className?: string;
}

/** Number that springs to its new value instead of jumping. */
export function AnimatedNumber({ value, format = defaultFormat, className }: AnimatedNumberProps) {
  const reduce = useReducedMotion();
  const spring = useSpring(value, { stiffness: 120, damping: 22, mass: 0.6 });
  const text = useTransform(spring, format);

  useEffect(() => {
    spring.set(value);
  }, [spring, value]);

  const cls = cn("tabular-nums", className);
  return reduce ? (
    <span className={cls}>{format(value)}</span>
  ) : (
    <motion.span className={cls}>{text}</motion.span>
  );
}
