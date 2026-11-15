import { useDeferredValue, useEffect, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { CommandPalette } from "@cloudflare/kumo/components/command-palette";
import {
  CompassIcon,
  CubeIcon,
  GearSixIcon,
  HardDrivesIcon,
  LightningIcon,
  MagnifyingGlassIcon,
  SquaresFourIcon,
  type Icon,
} from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { useTheme } from "@hearth/ui/theme";
import { api } from "../api/client";
import { navigate } from "../lib/url-state";

interface Hit {
  kind: "app" | "discover" | "container" | "drive" | "setting" | "action";
  id: string;
  title: string;
  subtitle: string;
  icon: string | null;
  score: number;
}

interface Group {
  id: string;
  label: string;
  items: Hit[];
}

const GROUPS: { kind: Hit["kind"]; label: string; icon: Icon }[] = [
  { kind: "action", label: "Actions", icon: LightningIcon },
  { kind: "app", label: "Your apps", icon: SquaresFourIcon },
  { kind: "container", label: "Containers", icon: CubeIcon },
  { kind: "setting", label: "Settings", icon: GearSixIcon },
  { kind: "drive", label: "Drives", icon: HardDrivesIcon },
  { kind: "discover", label: "Discover", icon: CompassIcon },
];

const SUGGESTED: Hit[] = [
  { kind: "action", id: "view:discover", title: "Discover 1,300+ apps", subtitle: "", icon: null, score: 0 },
  { kind: "setting", id: "settings:setup", title: "Setup checklist", subtitle: "", icon: null, score: 0 },
  { kind: "action", id: "tab:containers", title: "Containers", subtitle: "", icon: null, score: 0 },
  { kind: "action", id: "theme:toggle", title: "Toggle theme", subtitle: "", icon: null, score: 0 },
];

/** ⌘K / Ctrl+K: search everything (server-side Tantivy) and jump anywhere. */
export function GlobalSearch() {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const query = useDeferredValue(q.trim());
  const { cycle } = useTheme();
  const { data } = useQuery({
    queryKey: ["search", query],
    queryFn: () => api<{ hits: Hit[]; took_ms: number }>(`/search?q=${encodeURIComponent(query)}&limit=30`),
    enabled: open && query.length > 0,
    placeholderData: keepPreviousData,
    staleTime: 5_000,
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    const onOpen = () => setOpen(true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("hearth:search", onOpen);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("hearth:search", onOpen);
    };
  }, []);

  const hits = query ? (data?.hits ?? []) : SUGGESTED;
  const groups: Group[] = GROUPS.map((g) => ({ id: g.kind, label: g.label, items: hits.filter((h) => h.kind === g.kind).slice(0, g.kind === "discover" ? 8 : 6) })).filter(
    (g) => g.items.length > 0,
  );

  const select = (h: Hit) => {
    setOpen(false);
    setQ("");
    const [ns, val = ""] = h.id.split(":");
    switch (h.kind) {
      case "app":
        return navigate({ tab: "apps", view: "all", q: h.title });
      case "discover":
        return navigate({ tab: "apps", view: "discover", app: h.id });
      case "container":
        return navigate({ tab: "containers", container: h.id });
      case "drive":
        return navigate({ tab: "storage" });
      case "setting":
        return navigate({ tab: "settings", section: val });
      case "action":
        if (h.id === "theme:toggle") return cycle();
        if (ns === "tab") return navigate({ tab: val });
        if (ns === "view") return navigate({ tab: "apps", view: val });
        if (ns === "custom") return navigate({ tab: "apps", view: "all" });
    }
  };

  return (
    <CommandPalette.Root
      open={open}
      onOpenChange={setOpen}
      items={groups}
      value={q}
      onValueChange={setQ}
      filter={() => true}
      itemToStringValue={(g: Group) => g.label}
      getSelectableItems={(gs: Group[]) => gs.flatMap((g) => g.items)}
      onSelect={(item: Hit) => select(item)}
    >
      <CommandPalette.Input placeholder="Search apps, containers, settings…" leading={<MagnifyingGlassIcon size={16} aria-hidden="true" />} />
      <CommandPalette.List>
        <CommandPalette.Results>
          {(group: Group) => {
            const meta = GROUPS.find((g) => g.kind === group.id);
            const GIcon = meta?.icon ?? CompassIcon;
            return (
              <CommandPalette.Group key={group.id} items={group.items}>
                <CommandPalette.GroupLabel>{group.label}</CommandPalette.GroupLabel>
                <CommandPalette.Items>
                  {(item: Hit) => (
                    <CommandPalette.Item key={`${item.kind}:${item.id}`} value={item} onClick={() => select(item)}>
                      <span className="flex min-w-0 items-center gap-3">
                        {item.icon ? (
                          <AppLogo src={item.icon} name={item.title} category="" size="sm" className="size-6 rounded-md p-0.5" />
                        ) : (
                          <span className="inline-flex size-6 shrink-0 items-center justify-center text-kumo-subtle">
                            <GIcon size={16} aria-hidden="true" />
                          </span>
                        )}
                        <span className="min-w-0">
                          <span className="block truncate">{item.title}</span>
                          {item.subtitle ? <span className="block truncate text-xs text-kumo-subtle">{item.subtitle}</span> : null}
                        </span>
                      </span>
                    </CommandPalette.Item>
                  )}
                </CommandPalette.Items>
              </CommandPalette.Group>
            );
          }}
        </CommandPalette.Results>
        <CommandPalette.Empty>{query ? `Nothing found for “${query}”.` : "Start typing to search."}</CommandPalette.Empty>
      </CommandPalette.List>
      <CommandPalette.Footer>
        <span className="flex items-center gap-2">
          <kbd className="rounded border border-kumo-hairline bg-kumo-base px-1.5 py-0.5 text-[10px]">↑↓</kbd> Navigate
        </span>
        <span className="flex items-center gap-2">
          <kbd className="rounded border border-kumo-hairline bg-kumo-base px-1.5 py-0.5 text-[10px]">↵</kbd> Open
        </span>
        {data && query ? <span className="ml-auto tabular-nums">{data.took_ms.toFixed(1)} ms</span> : null}
      </CommandPalette.Footer>
    </CommandPalette.Root>
  );
}

/** Header button that opens the palette (also shows the shortcut). */
export function SearchButton() {
  const mac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
  return (
    <button
      type="button"
      onClick={() => window.dispatchEvent(new Event("hearth:search"))}
      className="flex h-8 items-center gap-2 rounded-lg px-2.5 text-sm text-kumo-subtle ring-1 ring-kumo-hairline transition-colors hover:bg-kumo-tint hover:text-kumo-default sm:w-56"
      aria-label="Search (Command K)"
    >
      <MagnifyingGlassIcon size={15} aria-hidden="true" />
      <span className="hidden flex-1 text-left sm:inline">Search…</span>
      <kbd className="hidden rounded border border-kumo-hairline px-1.5 text-[10px] sm:inline">{mac ? "⌘K" : "Ctrl K"}</kbd>
    </button>
  );
}
