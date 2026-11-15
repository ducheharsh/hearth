import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import type { Icon } from "@phosphor-icons/react";
import { cn } from "@cloudflare/kumo/utils";
import { springSnappy, springSoft } from "../motion";

export interface SettingsNavItem {
  id: string;
  label: string;
  icon: Icon;
  /** Small count/status shown on the right (e.g. "2"). */
  badge?: ReactNode;
}

interface SettingsLayoutProps {
  items: SettingsNavItem[];
  active: string;
  onSelect: (id: string) => void;
  children: ReactNode;
}

/** Two-pane settings: sticky section nav (horizontal on mobile) + one section at a time. */
export function SettingsLayout({ items, active, onSelect, children }: SettingsLayoutProps) {
  return (
    <div className="grid grid-cols-1 gap-8 lg:grid-cols-[13rem_minmax(0,1fr)]">
      <nav aria-label="Settings sections" className="lg:sticky lg:top-6 lg:self-start">
        <ul className="-mx-4 flex gap-1 overflow-x-auto px-4 pb-1 [scrollbar-width:none] lg:mx-0 lg:flex-col lg:px-0">
          {items.map((it) => {
            const on = it.id === active;
            const I = it.icon;
            return (
              <li key={it.id} className="shrink-0">
                <button
                  type="button"
                  onClick={() => onSelect(it.id)}
                  aria-current={on ? "page" : undefined}
                  className={cn(
                    "relative flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-colors",
                    on ? "text-kumo-default" : "text-kumo-subtle hover:bg-kumo-tint hover:text-kumo-default",
                  )}
                >
                  {on ? <motion.span layoutId="settings-nav" className="absolute inset-0 rounded-lg bg-kumo-tint ring-1 ring-kumo-hairline" transition={springSnappy} /> : null}
                  <I size={16} weight={on ? "fill" : "regular"} aria-hidden="true" className="relative" />
                  <span className="relative flex-1 whitespace-nowrap font-medium">{it.label}</span>
                  {it.badge != null ? <span className="relative text-xs tabular-nums text-kumo-subtle">{it.badge}</span> : null}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={active} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -4 }} transition={{ duration: 0.16 }} className="flex min-w-0 flex-col gap-8">
          {children}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

/** Page header for a settings section. */
export function SettingsHeader({ title, description }: { title: string; description?: string }) {
  return (
    <header>
      <h2 className="text-xl font-semibold tracking-tight">{title}</h2>
      {description ? <p className="mt-1 max-w-2xl text-sm text-kumo-subtle text-pretty">{description}</p> : null}
    </header>
  );
}

interface GroupProps {
  title: string;
  description?: ReactNode;
  /** Right side of the header (status badge, switch…). */
  aside?: ReactNode;
  /** Muted footer: help text left, actions right. */
  footer?: ReactNode;
  footerActions?: ReactNode;
  tone?: "default" | "danger";
  children?: ReactNode;
}

/** A titled card of setting rows. */
export function SettingsGroup({ title, description, aside, footer, footerActions, tone = "default", children }: GroupProps) {
  return (
    <section className={cn("overflow-hidden rounded-xl bg-kumo-base ring-1", tone === "danger" ? "ring-kumo-danger/40" : "ring-kumo-hairline")} aria-label={title}>
      <div className="flex items-start justify-between gap-4 px-5 pb-1 pt-5">
        <div className="min-w-0">
          <h3 className={cn("font-semibold", tone === "danger" && "text-kumo-danger")}>{title}</h3>
          {description ? <div className="mt-1 text-sm text-kumo-subtle text-pretty">{description}</div> : null}
        </div>
        {aside ? <div className="shrink-0">{aside}</div> : null}
      </div>
      {children ? <div className="divide-y divide-kumo-hairline px-5">{children}</div> : <div className="h-4" />}
      {footer || footerActions ? (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-kumo-hairline bg-kumo-elevated px-5 py-3 text-xs text-kumo-subtle">
          <div className="min-w-0 text-pretty">{footer}</div>
          {footerActions ? <div className="flex gap-2">{footerActions}</div> : null}
        </div>
      ) : null}
    </section>
  );
}

interface RowProps {
  label: string;
  description?: ReactNode;
  /** Associates the label with a control id. */
  htmlFor?: string;
  /** Put the control under the label (for wide inputs). */
  stack?: boolean;
  children?: ReactNode;
}

/** Label + explanation on the left, control on the right. */
export function SettingRow({ label, description, htmlFor, stack, children }: RowProps) {
  const Label = htmlFor ? "label" : "div";
  return (
    <div className={cn("gap-x-8 gap-y-3 py-4", stack ? "flex flex-col" : "grid sm:grid-cols-[minmax(0,1fr)_minmax(0,auto)] sm:items-center")}>
      <div className="min-w-0">
        <Label {...(htmlFor ? { htmlFor } : {})} className="text-sm font-medium text-kumo-default">
          {label}
        </Label>
        {description ? <div className="mt-0.5 text-xs text-kumo-subtle text-pretty">{description}</div> : null}
      </div>
      {children != null ? <div className={cn("min-w-0", !stack && "sm:justify-self-end")}>{children}</div> : null}
    </div>
  );
}

/** Sticky "unsaved changes" bar. */
export function SaveBar({ visible, onSave, onDiscard, saving, error }: { visible: boolean; onSave: () => void; onDiscard: () => void; saving?: boolean; error?: string | null }) {
  return (
    <AnimatePresence>
      {visible ? (
        <motion.div
          role="region"
          aria-label="Unsaved changes"
          initial={{ y: 80, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          exit={{ y: 80, opacity: 0 }}
          transition={springSoft}
          className="fixed inset-x-0 bottom-4 z-40 mx-auto flex w-[min(40rem,calc(100%-2rem))] items-center gap-3 rounded-xl bg-kumo-contrast px-4 py-3 text-sm text-kumo-inverse shadow-2xl"
        >
          <span className="flex-1" aria-live="polite">
            {error ? <span className="text-kumo-danger">{error}</span> : "You have unsaved changes."}
          </span>
          <button type="button" onClick={onDiscard} className="rounded-md px-3 py-1.5 font-medium opacity-80 hover:opacity-100">
            Discard
          </button>
          <button type="button" onClick={onSave} disabled={saving} className="rounded-md bg-kumo-brand px-3 py-1.5 font-medium text-white disabled:opacity-60">
            {saving ? "Saving…" : "Save Changes"}
          </button>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
