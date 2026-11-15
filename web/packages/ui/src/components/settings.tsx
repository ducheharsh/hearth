import { useEffect, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { CaretRightIcon, type Icon } from "@phosphor-icons/react";
import { cn } from "@cloudflare/kumo/utils";
import { springSnappy, springSoft } from "../motion";

export interface SettingsNavChild {
  /** Anchor id of the group (see `groupId`). */
  id: string;
  label: string;
}

export interface SettingsNavItem {
  id: string;
  label: string;
  icon: Icon;
  /** Small count/status shown on the right (e.g. "2"). */
  badge?: ReactNode;
  children?: SettingsNavChild[];
}

/** Stable anchor id for a group title, used by the tree and `SettingsGroup`. */
export function groupId(title: string): string {
  return `s-${title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`;
}

interface SettingsLayoutProps {
  items: SettingsNavItem[];
  active: string;
  onSelect: (id: string) => void;
  children: ReactNode;
}

/** Track which group of the current section is in view. */
function useScrollSpy(ids: string[]) {
  const [current, setCurrent] = useState<string | null>(null);
  const key = ids.join("|");
  useEffect(() => {
    if (ids.length === 0) return;
    const els = ids.map((id) => document.getElementById(id)).filter((e): e is HTMLElement => e != null);
    if (els.length === 0) return;
    const visible = new Map<string, number>();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) visible.set(e.target.id, e.isIntersecting ? e.intersectionRatio : 0);
        const best = ids.find((id) => (visible.get(id) ?? 0) > 0);
        if (best) setCurrent(best);
      },
      { rootMargin: "-80px 0px -55% 0px", threshold: [0, 0.01, 0.5] },
    );
    els.forEach((el) => io.observe(el));
    return () => io.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return current;
}

function Tree({ items, active, onSelect, current, onJump }: { items: SettingsNavItem[]; active: string; onSelect: (id: string) => void; current: string | null; onJump: (section: string, child: string) => void }) {
  const [open, setOpen] = useState<Set<string>>(() => new Set([active]));
  useEffect(() => setOpen((o) => new Set(o).add(active)), [active]);
  return (
    <ul className="flex flex-col gap-0.5" role="tree" aria-label="Settings">
      {items.map((it) => {
        const on = it.id === active;
        const expanded = open.has(it.id) && (it.children?.length ?? 0) > 0;
        const I = it.icon;
        return (
          <li key={it.id} role="treeitem" aria-expanded={it.children?.length ? expanded : undefined} aria-selected={on}>
            <div className="relative flex items-center">
              {on ? <motion.span layoutId="settings-nav" className="absolute inset-0 rounded-lg bg-kumo-tint" transition={springSnappy} /> : null}
              <button
                type="button"
                onClick={() => {
                  onSelect(it.id);
                  setOpen((o) => new Set(o).add(it.id));
                }}
                aria-current={on ? "page" : undefined}
                className={cn(
                  "relative flex min-w-0 flex-1 items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-colors",
                  on ? "font-medium text-kumo-default" : "text-kumo-subtle hover:text-kumo-default",
                )}
              >
                <I size={16} weight={on ? "fill" : "regular"} aria-hidden="true" />
                <span className="flex-1 truncate">{it.label}</span>
                {it.badge != null ? <span className="rounded-full bg-kumo-warning/20 px-1.5 text-[10px] font-semibold tabular-nums text-kumo-warning">{it.badge}</span> : null}
              </button>
              {it.children?.length ? (
                <button
                  type="button"
                  aria-label={expanded ? `Collapse ${it.label}` : `Expand ${it.label}`}
                  onClick={() =>
                    setOpen((o) => {
                      const n = new Set(o);
                      if (n.has(it.id)) n.delete(it.id);
                      else n.add(it.id);
                      return n;
                    })
                  }
                  className="relative mr-1 rounded-md p-1 text-kumo-subtle hover:bg-kumo-base hover:text-kumo-default"
                >
                  <motion.span animate={{ rotate: expanded ? 90 : 0 }} transition={springSnappy} className="inline-flex">
                    <CaretRightIcon size={12} aria-hidden="true" />
                  </motion.span>
                </button>
              ) : null}
            </div>
            <AnimatePresence initial={false}>
              {expanded ? (
                <motion.ul
                  role="group"
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.18 }}
                  className="ml-[1.35rem] overflow-hidden border-l border-kumo-hairline"
                >
                  {it.children?.map((c) => {
                    const here = on && current === c.id;
                    return (
                      <li key={c.id} role="treeitem" aria-selected={here}>
                        <button
                          type="button"
                          onClick={() => onJump(it.id, c.id)}
                          className={cn(
                            "-ml-px block w-full truncate border-l py-1.5 pl-3 pr-2 text-left text-[13px] transition-colors",
                            here ? "border-kumo-brand font-medium text-kumo-default" : "border-transparent text-kumo-subtle hover:border-kumo-line hover:text-kumo-default",
                          )}
                        >
                          {c.label}
                        </button>
                      </li>
                    );
                  })}
                </motion.ul>
              ) : null}
            </AnimatePresence>
          </li>
        );
      })}
    </ul>
  );
}

/** Cloudflare-style settings: collapsible section tree on the left, one section at a time. */
export function SettingsLayout({ items, active, onSelect, children }: SettingsLayoutProps) {
  const [pending, setPending] = useState<string | null>(null);
  const [mobileOpen, setMobileOpen] = useState(false);
  const childIds = items.find((i) => i.id === active)?.children?.map((c) => c.id) ?? [];
  const current = useScrollSpy(childIds);

  // After switching section, scroll to the requested group once it has rendered.
  useEffect(() => {
    if (!pending) return;
    const t = setTimeout(() => {
      document.getElementById(pending)?.scrollIntoView({ behavior: "smooth", block: "start" });
      setPending(null);
    }, 220);
    return () => clearTimeout(t);
  }, [pending, active]);

  const jump = (section: string, child: string) => {
    setMobileOpen(false);
    if (section !== active) onSelect(section);
    setPending(child);
  };
  const activeItem = items.find((i) => i.id === active);

  return (
    <div className="grid grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[15rem_minmax(0,1fr)] lg:gap-10">
      <nav aria-label="Settings sections" className="lg:sticky lg:top-6 lg:max-h-[calc(100dvh-3rem)] lg:self-start lg:overflow-y-auto lg:pr-1">
        <div className="lg:hidden">
          <button
            type="button"
            aria-expanded={mobileOpen}
            onClick={() => setMobileOpen((o) => !o)}
            className="flex w-full items-center justify-between rounded-xl bg-kumo-base px-4 py-3 text-sm font-medium ring-1 ring-kumo-hairline"
          >
            <span className="flex items-center gap-2">
              {activeItem ? <activeItem.icon size={16} aria-hidden="true" /> : null}
              {activeItem?.label ?? "Settings"}
            </span>
            <motion.span animate={{ rotate: mobileOpen ? 90 : 0 }} className="inline-flex text-kumo-subtle">
              <CaretRightIcon size={14} aria-hidden="true" />
            </motion.span>
          </button>
          <AnimatePresence initial={false}>
            {mobileOpen ? (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
                <div className="mt-2 rounded-xl bg-kumo-base p-2 ring-1 ring-kumo-hairline">
                  <Tree items={items} active={active} onSelect={(id) => { onSelect(id); setMobileOpen(false); }} current={current} onJump={jump} />
                </div>
              </motion.div>
            ) : null}
          </AnimatePresence>
        </div>
        <div className="hidden lg:block">
          <Tree items={items} active={active} onSelect={onSelect} current={current} onJump={jump} />
        </div>
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
    <section id={groupId(title)} className={cn("scroll-mt-24 overflow-hidden rounded-xl bg-kumo-base ring-1", tone === "danger" ? "ring-kumo-danger/40" : "ring-kumo-hairline")} aria-label={title}>
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
