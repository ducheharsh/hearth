import { useMemo, useState } from "react";
import { LayoutGroup, motion } from "motion/react";
import { Button } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import {
  BookOpenIcon,
  BrainIcon,
  ChartBarIcon,
  ChatsCircleIcon,
  CloudArrowUpIcon,
  CodeIcon,
  CurrencyCircleDollarIcon,
  DatabaseIcon,
  EnvelopeIcon,
  FilmStripIcon,
  FolderIcon,
  GameControllerIcon,
  HouseIcon,
  ImagesIcon,
  KeyIcon,
  ListChecksIcon,
  MagnifyingGlassIcon,
  MapPinIcon,
  MusicNotesIcon,
  NewspaperIcon,
  NotePencilIcon,
  PhoneIcon,
  RssIcon,
  ShieldCheckIcon,
  ShoppingCartIcon,
  SquaresFourIcon,
  TerminalWindowIcon,
  TreeStructureIcon,
  UsersThreeIcon,
  VideoCameraIcon,
  WifiHighIcon,
  XIcon,
  type Icon,
} from "@phosphor-icons/react";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { springSnappy } from "@hearth/ui/motion";

/** Keyword → icon. First match wins; covers awesome-selfhosted's tag vocabulary. */
const ICONS: [RegExp, Icon][] = [
  [/photo|gallery|image/i, ImagesIcon],
  [/video|stream|surveillance/i, VideoCameraIcon],
  [/music|audio|podcast/i, MusicNotesIcon],
  [/media|film|movie/i, FilmStripIcon],
  [/password|secret|identity|auth/i, KeyIcon],
  [/security|vpn|firewall/i, ShieldCheckIcon],
  [/dns|network|proxy|vpn/i, WifiHighIcon],
  [/ai|machine learning|artificial/i, BrainIcon],
  [/analytics|monitoring|metric|status/i, ChartBarIcon],
  [/chat|communication|irc|xmpp|matrix|social/i, ChatsCircleIcon],
  [/mail|email/i, EnvelopeIcon],
  [/voip|phone|sip/i, PhoneIcon],
  [/file|sync|storage|transfer/i, FolderIcon],
  [/backup|archiv/i, CloudArrowUpIcon],
  [/database|nosql|sql/i, DatabaseIcon],
  [/software development|ide|code|git|ci|api/i, CodeIcon],
  [/automation|workflow|orchestration/i, TreeStructureIcon],
  [/task|project management|todo|ticket/i, ListChecksIcon],
  [/note|wiki|knowledge|document|editor/i, NotePencilIcon],
  [/blog|cms|content|static site|publish/i, NewspaperIcon],
  [/feed|rss|news/i, RssIcon],
  [/book|e-?book|library|read/i, BookOpenIcon],
  [/money|finance|budget|accounting|invoice/i, CurrencyCircleDollarIcon],
  [/commerce|shop/i, ShoppingCartIcon],
  [/game/i, GameControllerIcon],
  [/home automation|iot|internet of things/i, HouseIcon],
  [/map|gps|location/i, MapPinIcon],
  [/search/i, MagnifyingGlassIcon],
  [/group|community|calendar|contact|crm|hr/i, UsersThreeIcon],
  [/server|admin|tool|remote|terminal/i, TerminalWindowIcon],
];

export function categoryIcon(tag: string): Icon {
  return ICONS.find(([re]) => re.test(tag))?.[1] ?? SquaresFourIcon;
}

interface CategoryProps {
  tags: [string, number][];
  value: string;
  onChange: (tag: string) => void;
}

/** Horizontal, scrollable rail of the top categories plus a "More" sheet. */
export function CategoryRail({ tags, value, onChange }: CategoryProps) {
  const [open, setOpen] = useState(false);
  const top = tags.slice(0, 12);
  const inRail = !value || top.some(([t]) => t === value);
  const chips: [string, number | null][] = [["", null], ...(inRail ? top : [[value, tags.find(([t]) => t === value)?.[1] ?? 0] as [string, number], ...top.slice(0, 11)])];
  return (
    <div className="flex min-w-0 items-center gap-2">
      <LayoutGroup id="discover-categories">
        <div role="toolbar" aria-label="Categories" className="-mx-4 flex min-w-0 flex-1 gap-1.5 overflow-x-auto px-4 py-1 [mask-image:linear-gradient(to_right,black_92%,transparent)] [scrollbar-width:none] sm:mx-0 sm:px-0">
          {chips.map(([t, n]) => {
            const on = t === value;
            const I = t ? categoryIcon(t) : SquaresFourIcon;
            return (
              <button
                key={t || "all"}
                type="button"
                aria-pressed={on}
                onClick={() => onChange(t)}
                className="relative flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-medium text-kumo-subtle ring-1 ring-kumo-hairline transition-colors hover:text-kumo-default aria-pressed:text-kumo-inverse aria-pressed:ring-transparent"
              >
                {on ? <motion.span layoutId="discover-chip" className="absolute inset-0 rounded-full bg-kumo-contrast" transition={springSnappy} /> : null}
                <I size={14} weight={on ? "fill" : "regular"} aria-hidden="true" className="relative" />
                <span className="relative whitespace-nowrap">{t || "All"}</span>
                {n != null ? <span className="relative tabular-nums opacity-60">{n}</span> : null}
              </button>
            );
          })}
        </div>
      </LayoutGroup>
      <Button size="sm" variant="secondary" icon={<SquaresFourIcon />} onClick={() => setOpen(true)} className="shrink-0">
        All {tags.length}
      </Button>
      {open ? (
        <CategorySheet
          tags={tags}
          value={value}
          onChange={(t) => {
            onChange(t);
            setOpen(false);
          }}
          onClose={() => setOpen(false)}
        />
      ) : null}
    </div>
  );
}

function CategorySheet({ tags, value, onChange, onClose }: CategoryProps & { onClose: () => void }) {
  const [q, setQ] = useState("");
  const shown = useMemo(() => {
    const n = q.trim().toLowerCase();
    return n ? tags.filter(([t]) => t.toLowerCase().includes(n)) : tags;
  }, [tags, q]);
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="xl" className={DIALOG_FRAME}>
        <DialogFrame
          header={
            <div className="flex flex-col gap-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <Dialog.Title className="text-lg font-semibold">Browse Categories</Dialog.Title>
                  <Dialog.Description className="text-sm text-kumo-subtle">{tags.length} categories from the self-hosted community.</Dialog.Description>
                </div>
                <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
              </div>
              <div className="relative">
                <MagnifyingGlassIcon size={16} aria-hidden="true" className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-kumo-subtle" />
                <input
                  aria-label="Filter categories"
                  type="text" inputMode="search" enterKeyHint="search"
                  autoComplete="off"
                  spellCheck={false}
                  placeholder="Filter categories…"
                  value={q}
                  onChange={(e) => setQ(e.target.value)}
                  className="h-10 w-full rounded-lg bg-kumo-base pl-9 pr-3 text-sm ring-1 ring-kumo-line placeholder:text-kumo-placeholder focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kumo-brand"
                />
              </div>
            </div>
          }
        >
          <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
            {shown.map(([t, n]) => {
              const I = categoryIcon(t);
              const on = t === value;
              return (
                <li key={t}>
                  <button
                    type="button"
                    aria-pressed={on}
                    onClick={() => onChange(on ? "" : t)}
                    className="flex w-full items-center gap-3 rounded-xl p-3 text-left ring-1 ring-kumo-hairline transition-[background-color,box-shadow] hover:bg-kumo-tint aria-pressed:bg-kumo-tint aria-pressed:ring-kumo-brand"
                  >
                    <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg bg-kumo-elevated ring-1 ring-kumo-hairline" aria-hidden="true">
                      <I size={18} weight="duotone" />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium">{t}</span>
                      <span className="block text-xs tabular-nums text-kumo-subtle">
                        {n} app{n === 1 ? "" : "s"}
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
          {shown.length === 0 ? <p className="py-6 text-center text-sm text-kumo-subtle">No categories match “{q}”.</p> : null}
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}

/** Big tiles for the Discover landing view (no filter yet). */
export function CategoryTiles({ tags, onChange }: { tags: [string, number][]; onChange: (t: string) => void }) {
  return (
    <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
      {tags.slice(0, 12).map(([t, n], i) => {
        const I = categoryIcon(t);
        return (
          <motion.li key={t} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0, transition: { delay: i * 0.03 } }}>
            <button
              type="button"
              onClick={() => onChange(t)}
              className="group flex h-full w-full flex-col items-start gap-3 rounded-xl bg-kumo-base p-4 text-left ring-1 ring-kumo-hairline transition-[box-shadow,transform] hover:-translate-y-0.5 hover:shadow-md hover:ring-kumo-line"
            >
              <span className="inline-flex size-10 items-center justify-center rounded-lg bg-kumo-elevated ring-1 ring-kumo-hairline transition-colors group-hover:bg-kumo-tint" aria-hidden="true">
                <I size={20} weight="duotone" />
              </span>
              <span className="min-w-0">
                <span className="line-clamp-2 text-sm font-medium text-balance">{t}</span>
                <span className="text-xs tabular-nums text-kumo-subtle">{n} apps</span>
              </span>
            </button>
          </motion.li>
        );
      })}
    </ul>
  );
}
