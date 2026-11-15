import { useDeferredValue } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Empty } from "@cloudflare/kumo/components/empty";
import { Input } from "@cloudflare/kumo/components/input";
import { Loader, SkeletonLine } from "@cloudflare/kumo/components/loader";
import { Switch } from "@cloudflare/kumo/components/switch";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { ArrowsClockwiseIcon, MagnifyingGlassIcon, PackageIcon, StarIcon, WarningIcon } from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { Card, InteractiveCard } from "@hearth/ui/components/card";
import { Select } from "@hearth/ui/components/form";
import { Section } from "@hearth/ui/components/section";
import { Stars } from "@hearth/ui/components/stars";
import { formatCompact, formatMonth, formatRelative } from "@hearth/ui/format";
import { riseIn } from "@hearth/ui/motion";
import { useDiscover, useDiscoverMeta, useDiscoverSync, useLeaderboards, useRate, type DiscoverQuery } from "../../api/hooks";
import type { DiscoverEntry } from "../../api/types";
import { useUrlParam } from "../../lib/url-state";
import { CategoryRail, CategoryTiles } from "./categories";
import { useOpenDiscoverApp } from "./discover-sheet";

const icon = (e: DiscoverEntry) => (e.has_icon ? `/api/icons/${e.slug}.webp` : null);

export function Discover() {
  const meta = useDiscoverMeta();
  const sync = useDiscoverSync();
  const [q, setQ] = useUrlParam<string>("dq", "");
  const [tag, setTag] = useUrlParam<string>("tag", "");
  const [sort, setSort] = useUrlParam<DiscoverQuery["sort"]>("sort", "stars");
  const [docker, setDocker] = useUrlParam<"1" | "0">("docker", "1");
  const [page, setPage] = useUrlParam<string>("page", "0");
  const query: DiscoverQuery = { q: useDeferredValue(q), tag, sort, docker_only: docker === "1", page: Number(page) || 0 };
  const results = useDiscover(query);
  const tags = Object.entries(meta.data?.tags ?? {}).toSorted((a, b) => b[1] - a[1]);

  if (meta.data && meta.data.total === 0) {
    return (
      <Empty
        icon={meta.data.syncing || sync.isPending ? <Loader size={32} /> : <PackageIcon size={40} />}
        title={meta.data.syncing || sync.isPending ? "Downloading the directory…" : "Directory not downloaded yet"}
        description={meta.data.error ?? "hearth syncs 1,300+ self-hosted apps from awesome-selfhosted."}
        contents={
          <Button variant="primary" icon={<ArrowsClockwiseIcon />} loading={sync.isPending} onClick={() => sync.mutate()}>
            Download Now
          </Button>
        }
      />
    );
  }

  return (
    <div className="flex flex-col gap-10">
      <DirectoryLeaderboards />

      <Section
        title="Browse Everything"
        description={meta.data ? `${meta.data.total.toLocaleString()} apps · synced ${formatRelative(meta.data.synced_at)}` : undefined}
        actions={
          <Button size="sm" variant="ghost" icon={<ArrowsClockwiseIcon />} loading={sync.isPending} onClick={() => sync.mutate()}>
            Refresh
          </Button>
        }
      >
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center">
          <div className="relative lg:w-72">
            <MagnifyingGlassIcon size={16} aria-hidden="true" className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-kumo-subtle" />
            <Input
              aria-label="Search the directory"
              type="text" inputMode="search" enterKeyHint="search"
              name="discover-search"
              autoComplete="off"
              spellCheck={false}
              placeholder="Search 1,300+ apps…"
              value={q}
              onChange={(e) => {
                setQ(e.target.value);
                setPage("0");
              }}
              className="w-full pl-8"
            />
          </div>

          <Select
            aria-label="Sort by"
            className="lg:w-44"
            value={sort}
            onChange={(v) => setSort(v as DiscoverQuery["sort"])}
            items={{ stars: "Most stars", activity: "Most active", updated: "Recently updated", name: "Name" }}
          />
          <Switch label="Docker only" checked={docker === "1"} onCheckedChange={(v) => setDocker(v ? "1" : "0")} />
        </div>

        <CategoryRail
          tags={tags}
          value={tag}
          onChange={(t) => {
            setTag(t);
            setPage("0");
          }}
        />

        {!q && !tag && tags.length ? (
          <div className="flex flex-col gap-3">
            <h3 className="text-sm font-medium">Browse by Category</h3>
            <CategoryTiles
              tags={tags}
              onChange={(t) => {
                setTag(t);
                setPage("0");
              }}
            />
          </div>
        ) : null}

        {results.error ? (
          <Banner variant="error" icon={<WarningIcon />} title="Search failed" description={results.error.message} />
        ) : !results.data ? (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {[0, 1, 2].map((i) => (
              <Card key={i} className="flex flex-col gap-3">
                <SkeletonLine />
                <SkeletonLine />
              </Card>
            ))}
          </div>
        ) : results.data.items.length === 0 ? (
          <Empty icon={<PackageIcon size={40} />} title="No matches" description="Try a broader search or another category." />
        ) : (
          <>
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-busy={results.isFetching}>
              <AnimatePresence mode="popLayout">
                {results.data.items.map(({ entry, rating }, i) => (
                  <DirectoryCard key={entry.slug} e={entry} rating={rating} index={i} />
                ))}
              </AnimatePresence>
            </div>
            <Pager total={results.data.total} perPage={results.data.per_page} page={query.page} onPage={(p) => setPage(String(p))} />
          </>
        )}
        {meta.data ? (
          <p className="text-xs text-kumo-subtle">
            Directory data from{" "}
            <a href={meta.data.source} target="_blank" rel="noopener noreferrer" className="underline">
              awesome-selfhosted
            </a>{" "}
            (CC&nbsp;BY-SA&nbsp;3.0). One-click install is available for apps under One-Click; the rest link to their projects.
          </p>
        ) : null}
      </Section>
    </div>
  );
}

function Pager({ total, perPage, page, onPage }: { total: number; perPage: number; page: number; onPage: (p: number) => void }) {
  const pages = Math.max(1, Math.ceil(total / perPage));
  return (
    <nav className="flex items-center justify-between gap-3 text-sm" aria-label="Pagination">
      <span className="tabular-nums text-kumo-subtle">
        {(page * perPage + 1).toLocaleString()}–{Math.min(total, (page + 1) * perPage).toLocaleString()} of {total.toLocaleString()}
      </span>
      <div className="flex gap-2">
        <Button size="sm" disabled={page === 0} onClick={() => onPage(page - 1)}>
          Previous
        </Button>
        <Button size="sm" disabled={page + 1 >= pages} onClick={() => onPage(page + 1)}>
          Next
        </Button>
      </div>
    </nav>
  );
}

function DirectoryCard({ e, rating, index }: { e: DiscoverEntry; rating: number | null; index: number }) {
  const rate = useRate();
  const open = useOpenDiscoverApp();
  return (
    <motion.div layout variants={riseIn} custom={index} initial="hidden" animate="show" exit="exit">
      <InteractiveCard className="flex h-full flex-col gap-3">
        <button type="button" onClick={() => open(e.slug)} className="flex items-start gap-3 rounded-lg text-left" aria-label={`${e.name} details`}>
          <AppLogo src={icon(e)} name={e.name} category={e.tags[0]?.toLowerCase() ?? ""} />
          <div className="min-w-0 flex-1">
            <h3 className="truncate font-semibold">{e.name}</h3>
            <p className="truncate text-xs text-kumo-subtle">{e.tags.join(" · ")}</p>
          </div>
          {e.stars != null ? (
            <span className="inline-flex items-center gap-1 text-xs tabular-nums text-kumo-subtle">
              <StarIcon size={12} weight="fill" aria-hidden="true" /> {formatCompact(e.stars)}
            </span>
          ) : null}
        </button>
        <p className="line-clamp-3 text-sm text-kumo-strong text-pretty">{e.description}</p>
        <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-kumo-subtle">
          <span>{e.licenses.join(", ") || "No license listed"}</span>
          {e.release ? <span translate="no">{e.release.tag}</span> : null}
          <span>updated {formatRelative(e.updated_at)}</span>
          {e.depends_3rdparty ? <span className="text-kumo-warning">needs a 3rd-party service</span> : null}
        </div>
        <div className="mt-auto flex flex-wrap items-center justify-between gap-2 pt-1">
          <Button size="sm" variant="secondary" onClick={() => open(e.slug)}>
            View Details
          </Button>
          <Stars value={rating} size={14} onChange={(stars) => rate.mutate({ key: e.slug, stars })} label={`Rate ${e.name}`} />
        </div>
      </InteractiveCard>
    </motion.div>
  );
}

type Board = "top_month" | "most_starred" | "rising" | "fresh_releases";

function DirectoryLeaderboards() {
  const { data } = useLeaderboards();
  const open = useOpenDiscoverApp();
  const [board, setBoard] = useUrlParam<Board>("dboard", "top_month");
  if (!data || data.top_month.length === 0) return null;
  const list = data[board];
  const detail = (e: DiscoverEntry) =>
    board === "top_month"
      ? `${formatCompact(e.commits[data.month] ?? 0)} commits in ${formatMonth(data.month)}`
      : board === "fresh_releases"
        ? `${e.release?.tag ?? ""} · ${formatRelative(e.release?.published_at)}`
        : board === "rising"
          ? `★ ${formatCompact(e.stars)} · fast-moving`
          : `★ ${formatCompact(e.stars)}`;
  return (
    <Section
      title="Charts"
      description="Live rankings from the whole self-hosted community."
      actions={
        <Tabs
          variant="underline"
          size="sm"
          value={board}
          onValueChange={(v) => setBoard(v as Board)}
          tabs={[
            { value: "top_month", label: `Top of ${formatMonth(data.month)}` },
            { value: "most_starred", label: "All-Time" },
            { value: "rising", label: "Rising" },
            { value: "fresh_releases", label: "New Releases" },
          ]}
        />
      }
    >
      <Card className="py-1">
        <ol className="grid grid-cols-1 sm:grid-cols-2 sm:gap-x-8">
          {list.map((e, i) => (
            <li key={e.slug} className="flex items-center gap-3 py-2">
              <span className="w-6 text-right text-lg font-semibold tabular-nums text-kumo-subtle">{i + 1}</span>
              <AppLogo src={icon(e)} name={e.name} category="" size="sm" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">{e.name}</span>
                <span className="block truncate text-xs text-kumo-subtle">{detail(e)}</span>
              </span>
              <Button size="xs" variant="secondary" onClick={() => open(e.slug)}>
                View
              </Button>
            </li>
          ))}
        </ol>
      </Card>
    </Section>
  );
}
