import {
  createContext,
  use,
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { AnimatePresence, motion } from "motion/react";
import { Button } from "@cloudflare/kumo/components/button";
import { DesktopIcon, MoonIcon, SunIcon } from "@phosphor-icons/react";
import { springSnappy } from "./motion";

export type ThemePreference = "system" | "light" | "dark";
type Resolved = "light" | "dark";

const STORAGE_KEY = "hearth:theme:v1";
const ORDER: readonly ThemePreference[] = ["system", "light", "dark"];

function readStored(): ThemePreference {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

const media = () => window.matchMedia("(prefers-color-scheme: dark)");
function subscribeSystem(cb: () => void) {
  const m = media();
  m.addEventListener("change", cb);
  return () => m.removeEventListener("change", cb);
}
const systemDark = () => media().matches;

interface ThemeContextValue {
  preference: ThemePreference;
  resolved: Resolved;
  cycle: () => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

/**
 * Applies `data-mode` on <html> (what Kumo reads). Pair with the inline script in
 * index.html that sets it before first paint, so there is no flash.
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState<ThemePreference>(readStored);
  const isSystemDark = useSyncExternalStore(subscribeSystem, systemDark, () => false);
  const resolved: Resolved =
    preference === "system" ? (isSystemDark ? "dark" : "light") : preference;

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.mode = resolved;
    const bg = getComputedStyle(document.body).backgroundColor;
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", bg);
  }, [resolved]);

  const cycle = useCallback(() => {
    setPreference((p) => {
      const next = ORDER[(ORDER.indexOf(p) + 1) % ORDER.length] ?? "system";
      try {
        localStorage.setItem(STORAGE_KEY, next);
      } catch {
        /* private mode: keep in memory */
      }
      return next;
    });
  }, []);

  const value = useMemo(() => ({ preference, resolved, cycle }), [preference, resolved, cycle]);
  return <ThemeContext value={value}>{children}</ThemeContext>;
}

export function useTheme(): ThemeContextValue {
  const ctx = use(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used inside <ThemeProvider>");
  return ctx;
}

const ICONS = { system: DesktopIcon, light: SunIcon, dark: MoonIcon } as const;
const LABELS = { system: "System theme", light: "Light theme", dark: "Dark theme" } as const;

/** Cycles system → light → dark with an icon that rotates in. */
export function ThemeToggle() {
  const { preference, cycle } = useTheme();
  const Icon = ICONS[preference];
  return (
    <Button
      variant="ghost"
      shape="square"
      onClick={cycle}
      aria-label={`${LABELS[preference]}. Switch theme`}
      title={LABELS[preference]}
    >
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.span
          key={preference}
          initial={{ rotate: -90, scale: 0.5, opacity: 0 }}
          animate={{ rotate: 0, scale: 1, opacity: 1 }}
          exit={{ rotate: 90, scale: 0.5, opacity: 0 }}
          transition={springSnappy}
          className="inline-flex"
        >
          <Icon size={18} aria-hidden="true" />
        </motion.span>
      </AnimatePresence>
    </Button>
  );
}
