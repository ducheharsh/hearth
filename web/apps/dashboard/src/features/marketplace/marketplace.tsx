import { useDeferredValue, useMemo, useState } from "react";
import { AnimatePresence, LayoutGroup, motion } from "motion/react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Empty } from "@cloudflare/kumo/components/empty";
import { Input } from "@cloudflare/kumo/components/input";
import { SkeletonLine } from "@cloudflare/kumo/components/loader";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { MagnifyingGlassIcon, PackageIcon, PlusIcon, SparkleIcon, WarningIcon } from "@phosphor-icons/react";

import { AppLogo } from "@hearth/ui/components/app-logo";
import { Card } from "@hearth/ui/components/card";
import { Pressable } from "@hearth/ui/components/pressable";
import { Section } from "@hearth/ui/components/section";
import { Packs } from "../packs/packs";
import { formatCompact, formatMonth } from "@hearth/ui/format";
import { springSnappy } from "@hearth/ui/motion";
import { useCatalog } from "../../api/hooks";
import type { CatalogApp } from "../../api/types";
import { useUrlParam } from "../../lib/url-state";
import { AppCard, AppRow, useAppDialogs } from "./app-card";
import { CustomAppDialog } from "./custom-app-dialog";
import { Discover } from "./discover";

const ACRONYMS: Record<string, string> = { ai: "AI" };
const categoryLabel = (c: string) => ACRONYMS[c] ?? c;

type View = "store" | "all" | "discover";

export function Marketplace() {
  const [view, setView] = useUrlParam<View>("view", "store");
  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Tabs
          variant="segmented"
          value={view}
          onValueChange={(v) => setView(v as View)}
          tabs={[
            { value: "store", label: "Today" },
            { value: "all", label: "One-Click" },
            { value: "discover", label: "Discover 1,300+" },
          ]}
        />
        <AddYourOwn />
      </div>
      {view === "discover" ? <Discover /> : view === "all" ? <AllApps /> : <Store />}
    </div>
  );
}

function AddYourOwn() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Pressable>
        <Button icon={<PlusIcon />} onClick={() => setOpen(true)}>
          Add Your Own
        </Button>
      </Pressable>
      {open ? <CustomAppDialog open onOpenChange={setOpen} /> : null}
    </>
  );
}

/** App Store "Today": editor's picks, top of the month, recommended, leaderboards. */
function Store() {
  const { data, error, isPending } = useCatalog();
  const apps = data?.apps ?? [];
  const month = data?.month ?? null;

  const featured = apps.filter((a) => a.featured);
  const topMonth = apps
    .filter((a) => a.stats)
    .toSorted((a, b) => (b.stats?.commits_this_month ?? 0) - (a.stats?.commits_this_month ?? 0))
    .slice(0, 5);
  const recommended = apps
    .filter((a) => !a.installed && a.verdict?.fit === "ok")
    .toSorted((a, b) => a.ram_mb - b.ram_mb)
    .slice(0, 6);
  const popular = apps.filter((a) => a.stats?.stars != null).toSorted((a, b) => (b.stats?.stars ?? 0) - (a.stats?.stars ?? 0));
  const topRated = apps.filter((a) => a.rating).toSorted((a, b) => (b.rating ?? 0) - (a.rating ?? 0));
  const lightest = apps.toSorted((a, b) => a.ram_mb - b.ram_mb);

  if (error) return <Banner variant="error" icon={<WarningIcon />} title="Couldn’t load apps" description={error.message} />;
  if (isPending) return <SkeletonGrid />;

  return (
    <div className="flex flex-col gap-12">
      {featured.length > 0 ? <Hero apps={featured} /> : null}

      <Packs />

      <div className="grid grid-cols-1 gap-8 lg:grid-cols-2">
        <Section title={`Top 5 in ${formatMonth(month)}`} description="Most actively developed this month, by commits.">
          <Card className="py-1">
            <ol className="divide-y divide-kumo-hairline">
              {topMonth.map((a, i) => (
                <AppRow key={a.id} app={a} rank={i + 1} detail={`${formatCompact(a.stats?.commits_this_month)} commits · ★ ${formatCompact(a.stats?.stars)}`} />
              ))}
            </ol>
          </Card>
        </Section>
        <Section title="Recommended for This Mac" description="Fits comfortably in the memory you have right now.">
          {recommended.length === 0 ? (
            <Card className="text-sm text-kumo-subtle">Nothing fits comfortably right now. Free up memory or disk, or check the capacity notes on each app.</Card>
          ) : (
            <Card className="py-1">
              <ul className="divide-y divide-kumo-hairline">
                {recommended.map((a) => (
                  <AppRow key={a.id} app={a} detail={`~${a.ram_mb} MB RAM`} />
                ))}
              </ul>
            </Card>
          )}
        </Section>
      </div>

      <Leaderboards popular={popular} topRated={topRated} lightest={lightest} />
    </div>
  );
}

function Hero({ apps }: { apps: CatalogApp[] }) {
  return (
    <section aria-label="Must-have essentials" className="flex flex-col gap-3">
      <h2 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-kumo-subtle">
        <SparkleIcon size={14} weight="fill" aria-hidden="true" /> Must-Have Essentials
      </h2>
      <div className="-mx-4 flex snap-x snap-mandatory gap-4 overflow-x-auto px-4 pb-2 [scrollbar-width:none] sm:mx-0 sm:px-0">
        {apps.map((a, i) => (
          <HeroCard key={a.id} app={a} index={i} />
        ))}
      </div>
    </section>
  );
}

function HeroCard({ app, index }: { app: CatalogApp; index: number }) {
  const dialogs = useAppDialogs(app);
  return (
    <motion.article
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0, transition: { delay: index * 0.06 } }}
      whileHover={{ y: -3 }}
      className="relative flex w-[min(20rem,80vw)] shrink-0 snap-start flex-col gap-4 overflow-hidden rounded-2xl bg-kumo-base p-5 shadow-sm ring-1 ring-kumo-hairline"
    >
      <div aria-hidden="true" className="absolute -right-10 -top-10 size-40 rounded-full bg-kumo-brand opacity-10 blur-2xl" />
      <AppLogo src={app.icon} name={app.name} category={app.category} size="lg" />
      <div className="min-w-0">
        <h3 className="text-lg font-semibold tracking-tight">{app.name}</h3>
        <p className="mt-1 line-clamp-3 text-sm text-kumo-strong text-pretty">{app.editors_note ?? app.description}</p>
      </div>
      <div className="mt-auto flex items-center gap-2">
        <Pressable>
          <Button variant="primary" size="sm" onClick={() => dialogs.open(app.installed ? "sheet" : "install")}>
            {app.installed ? "Installed" : "Get"}
          </Button>
        </Pressable>
        <Button variant="ghost" size="sm" onClick={() => dialogs.open("sheet")}>
          Learn More
        </Button>
      </div>
      {dialogs.node}
    </motion.article>
  );
}

type Board = "popular" | "rated" | "light";

function Leaderboards({ popular, topRated, lightest }: { popular: CatalogApp[]; topRated: CatalogApp[]; lightest: CatalogApp[] }) {
  const [board, setBoard] = useUrlParam<Board>("board", "popular");
  const list = board === "rated" ? topRated : board === "light" ? lightest : popular;
  return (
    <Section
      title="Leaderboards"
      actions={
        <Tabs
          variant="underline"
          size="sm"
          value={board}
          onValueChange={(v) => setBoard(v as Board)}
          tabs={[
            { value: "popular", label: "Most Popular" },
            { value: "rated", label: "Your Top Rated" },
            { value: "light", label: "Lightest" },
          ]}
        />
      }
    >
      {list.length === 0 ? (
        <Card className="text-sm text-kumo-subtle">Rate apps from their details page to build your own top list.</Card>
      ) : (
        <Card className="py-1">
          <ol className="grid grid-cols-1 sm:grid-cols-2 sm:gap-x-8">
            {list.slice(0, 10).map((a, i) => (
              <AppRow
                key={a.id}
                app={a}
                rank={i + 1}
                detail={board === "light" ? `~${a.ram_mb} MB RAM` : board === "rated" ? `${a.rating} ★ from you` : `★ ${formatCompact(a.stats?.stars)} on GitHub`}
              />
            ))}
          </ol>
        </Card>
      )}
    </Section>
  );
}

function AllApps() {
  const { data, error, isPending } = useCatalog();
  const apps = data?.apps;
  const [query, setQuery] = useUrlParam<string>("q", "");
  const [category, setCategory] = useUrlParam<string>("category", "all");
  const deferredQuery = useDeferredValue(query);

  const categories = useMemo(() => ["all", ...[...new Set((apps ?? []).map((a) => (a.custom ? "custom" : a.category)))].toSorted()], [apps]);
  const visible = useMemo(() => {
    const q = deferredQuery.trim().toLowerCase();
    return (apps ?? []).filter(
      (a) =>
        (category === "all" || a.category === category || (category === "custom" && a.custom)) &&
        (!q || a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q)),
    );
  }, [apps, category, deferredQuery]);

  if (error) return <Banner variant="error" icon={<WarningIcon />} title="Couldn’t load apps" description={error.message} />;

  return (
    <Section title="One-Click Apps" description="Checked against this machine’s capacity and published behind your login.">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="relative sm:w-72">
          <MagnifyingGlassIcon size={16} aria-hidden="true" className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-kumo-subtle" />
          <Input aria-label="Search apps" name="search" type="text" inputMode="search" enterKeyHint="search" autoComplete="off" spellCheck={false} placeholder="Search apps…" value={query} onChange={(e) => setQuery(e.target.value)} className="w-full pl-8" />
        </div>
        <LayoutGroup id="categories">
          <div className="flex flex-wrap gap-1" role="toolbar" aria-label="Filter by category">
            {categories.map((c) => {
              const active = c === category;
              return (
                <button
                  key={c}
                  type="button"
                  aria-pressed={active}
                  onClick={() => setCategory(c)}
                  className="relative rounded-full px-3 py-1 text-xs font-medium capitalize text-kumo-subtle transition-colors hover:text-kumo-default aria-pressed:text-kumo-inverse"
                >
                  {active ? <motion.span layoutId="category-pill" className="absolute inset-0 rounded-full bg-kumo-contrast" transition={springSnappy} /> : null}
                  <span className="relative">{categoryLabel(c)}</span>
                </button>
              );
            })}
          </div>
        </LayoutGroup>
      </div>
      {isPending ? (
        <SkeletonGrid />
      ) : visible.length === 0 ? (
        <Empty icon={<PackageIcon size={40} />} title="No matching apps" description="Try another search, or browse Discover for 1,300+ more." />
      ) : (
        <motion.div layout className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <AnimatePresence mode="popLayout">
            {visible.map((app, i) => (
              <AppCard key={app.id} app={app} index={i} />
            ))}
          </AnimatePresence>
        </motion.div>
      )}
    </Section>
  );
}

function SkeletonGrid() {
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {[0, 1, 2, 3, 4, 5].map((i) => (
        <Card key={i} className="flex flex-col gap-3">
          <SkeletonLine />
          <SkeletonLine />
        </Card>
      ))}
    </div>
  );
}

