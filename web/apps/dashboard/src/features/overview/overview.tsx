import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { SkeletonLine } from "@cloudflare/kumo/components/loader";
import { useQuery } from "@tanstack/react-query";
import {
  CaretDownIcon,
  ArrowRightIcon,
  CheckCircleIcon,
  CubeIcon,
  HardDrivesIcon,
  PlusIcon,
  WarningCircleIcon,
  WarningIcon,
} from "@phosphor-icons/react";
import { AnimatedNumber } from "@hearth/ui/components/animated-number";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { Card } from "@hearth/ui/components/card";
import { RadialGauge } from "@hearth/ui/components/radial-gauge";
import { Section } from "@hearth/ui/components/section";
import { Sparkline } from "@hearth/ui/components/sparkline";
import { StatusDot } from "@hearth/ui/components/status-dot";
import {
  formatBytes,
  formatDuration,
  formatNumber,
  formatPercent,
  formatRelative,
} from "@hearth/ui/format";
import { riseIn, springSnappy } from "@hearth/ui/motion";
import { api } from "../../api/client";
import {
  useCatalog,
  useContainers,
  useSetup,
  useStorage,
  useSystem,
  useTailscale,
  useUpdates,
} from "../../api/hooks";
import type { HearthEvent, SystemSample } from "../../api/types";
import { navigate } from "../../lib/url-state";
import { AccessBar } from "./access-bar";

const pct = (n: number) => formatPercent(n);
const bytes = (n: number) => formatBytes(n);

interface Issue {
  text: string;
  action: string;
  go: Record<string, string>;
}

function useIssues(s: SystemSample | null | undefined): Issue[] {
  const setup = useSetup();
  const storage = useStorage();
  const issues: Issue[] = [];
  for (const a of storage.data?.attachments ?? []) {
    if (a.status.state === "offline")
      issues.push({
        text: `${a.label} (${a.role} drive) is disconnected; its apps are paused.`,
        action: "Storage",
        go: { tab: "storage" },
      });
  }
  if (s) {
    const root = s.disks.find((d) => d.mount === "/");
    if (root && root.free / root.total < 0.08)
      issues.push({
        text: `Internal disk is almost full (${formatBytes(root.free)} free).`,
        action: "Details",
        go: { tab: "overview" },
      });
    if (s.swap_total > 0 && s.swap_used / s.swap_total > 0.6)
      issues.push({
        text: "Memory is under pressure (heavy swapping). Stop an app or add RAM headroom.",
        action: "Containers",
        go: { tab: "containers" },
      });
  }
  const updates = useUpdates();
  const pinned = Object.values(updates.data?.apps ?? {}).filter(
    (a) => a.pinned,
  ).length;
  const avail = Object.values(updates.data?.apps ?? {}).filter(
    (a) => a.available && !a.pinned,
  ).length;
  if (pinned)
    issues.push({
      text: `${pinned} app${pinned > 1 ? "s were" : " was"} rolled back after an unhealthy update.`,
      action: "Apps",
      go: { tab: "apps", view: "all" },
    });
  if (avail)
    issues.push({
      text: `${avail} app update${avail > 1 ? "s" : ""} available.`,
      action: "Review",
      go: { tab: "apps", view: "all" },
    });
  const todo = setup.data?.filter((c) => c.state === "todo").length ?? 0;
  if (todo > 0)
    issues.push({
      text: `${todo} setup step${todo > 1 ? "s" : ""} left before your apps are reachable from your devices.`,
      action: "Finish setup",
      go: { tab: "settings", section: "setup" },
    });
  return issues;
}

export function Overview() {
  const { data, error, isPending } = useSystem();
  const s = data?.latest;
  const cpuSeries = useMemo(
    () => data?.history.map((p) => p.cpu) ?? [],
    [data?.history],
  );
  const memSeries = useMemo(
    () => data?.history.map((p) => p.mem_used) ?? [],
    [data?.history],
  );

  if (error) {
    return (
      <Banner
        variant="error"
        icon={<WarningIcon />}
        title="hearth isn’t running"
        description="Open the Hearth app (it starts it), or run `hearthd service install` / `brew services start hearth`."
      />
    );
  }
  if (isPending || !s) {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        {[0, 1, 2].map((i) => (
          <Card key={i} className="flex flex-col gap-3">
            <SkeletonLine />
            <SkeletonLine />
          </Card>
        ))}
      </div>
    );
  }
  const root = s.disks.find((d) => d.mount === "/");
  const diskPct = root ? ((root.total - root.free) / root.total) * 100 : 0;

  return (
    <div className="flex flex-col gap-10">
      <AccessBar />
      <Launcher />
      <Hero s={s} name={data.display_name} />

      <motion.div
        className="grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]"
        initial="hidden"
        animate="show"
      >
        <motion.div variants={riseIn} custom={0}>
          <Card className="grid h-full grid-cols-3 place-items-center gap-1 px-2 py-6 sm:gap-2 sm:px-4">
            <RadialGauge
              percent={s.cpu_percent}
              label="CPU"
              value={<AnimatedNumber value={s.cpu_percent} format={pct} />}
              detail={`${s.cpu_count} cores`}
            />
            <RadialGauge
              percent={(s.mem_used / s.mem_total) * 100}
              label="Memory"
              detail={`${formatBytes(s.mem_available)} free`}
            />
            <RadialGauge
              percent={diskPct}
              label="Disk"
              detail={root ? `${formatBytes(root.free)} free` : undefined}
            />
          </Card>
        </motion.div>
        <motion.div variants={riseIn} custom={1}>
          <Card className="flex h-full flex-col gap-4">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <h2 className="text-sm font-medium">Last hour</h2>
              <span className="text-xs text-kumo-subtle tabular-nums">
                load {s.load_avg.map((l) => formatNumber(l)).join(" · ")} · swap{" "}
                {formatBytes(s.swap_used)}
              </span>
            </div>
            <div className="grid grid-cols-1 flex-1 gap-4 sm:grid-cols-2">
              <div className="flex min-w-0 flex-col gap-1">
                <span className="text-xs text-kumo-subtle">
                  CPU{" "}
                  <AnimatedNumber
                    value={s.cpu_percent}
                    format={pct}
                    className="font-medium text-kumo-default"
                  />
                </span>
                <Sparkline
                  values={cpuSeries}
                  max={100}
                  label="CPU usage, last hour"
                  className="h-24"
                />
              </div>
              <div className="flex min-w-0 flex-col gap-1">
                <span className="text-xs text-kumo-subtle">
                  Memory{" "}
                  <AnimatedNumber
                    value={s.mem_used}
                    format={bytes}
                    className="font-medium text-kumo-default"
                  />
                </span>
                <Sparkline
                  values={memSeries}
                  max={s.mem_total}
                  tone="success"
                  label="Memory usage, last hour"
                  className="h-24"
                />
              </div>
            </div>
          </Card>
        </motion.div>
      </motion.div>

      <div className="grid grid-cols-1 gap-8 lg:grid-cols-2">
        <TopConsumers />
        <Activity />
      </div>
    </div>
  );
}

const HERO_KEY = "hearth-overview-issues";

function useCollapsed(): [boolean, (v: boolean) => void] {
  const [collapsed, set] = useState(() => {
    try {
      return localStorage.getItem(HERO_KEY) === "collapsed";
    } catch {
      return false;
    }
  });
  const update = (v: boolean) => {
    set(v);
    try {
      localStorage.setItem(HERO_KEY, v ? "collapsed" : "open");
    } catch {
      /* per-browser convenience only */
    }
  };
  return [collapsed, update];
}

function Hero({ s, name }: { s: SystemSample; name: string }) {
  const issues = useIssues(s);
  const ts = useTailscale();
  const catalog = useCatalog();
  const [collapsed, setCollapsed] = useCollapsed();
  const running = catalog.data?.apps.filter((a) => a.running).length ?? 0;
  const ok = issues.length === 0;
  const open = !ok && !collapsed;
  const heading = (
    <div className="flex min-w-0 flex-1 items-start gap-3">
      {ok ? (
        <CheckCircleIcon
          size={28}
          weight="fill"
          className="shrink-0 text-kumo-success"
          aria-hidden="true"
        />
      ) : (
        <WarningCircleIcon
          size={28}
          weight="fill"
          className="shrink-0 text-kumo-warning"
          aria-hidden="true"
        />
      )}
      <div className="min-w-0">
        <h1 className="text-2xl font-semibold tracking-tight text-balance">
          {ok ? `${name} is healthy` : `${name} needs attention`}
        </h1>
        <p className="mt-1 text-sm text-kumo-subtle text-pretty">
          Up {formatDuration(s.uptime_secs)} · {running} app
          {running === 1 ? "" : "s"} running
          {ts.data?.dns_name ? (
            <>
              {" · "}
              <span className="font-mono" translate="no">
                {ts.data.dns_name}
              </span>
            </>
          ) : null}
        </p>
      </div>
    </div>
  );
  return (
    <section
      aria-label="Status"
      data-tour="health"
      className={`relative overflow-hidden rounded-2xl bg-kumo-base ring-1 ring-kumo-hairline transition-[padding] ${open ? "p-6 sm:p-8" : "px-6 py-5 sm:px-8"}`}
    >
      <div
        aria-hidden="true"
        className={`absolute -right-16 -top-24 size-72 rounded-full blur-3xl ${ok ? "bg-kumo-success opacity-15" : "bg-kumo-warning opacity-20"}`}
      />
      <div className="relative flex flex-col">
        {ok ? (
          heading
        ) : (
          <button
            type="button"
            onClick={() => setCollapsed(!collapsed)}
            aria-expanded={open}
            aria-controls="overview-issues"
            className="-m-2 flex items-start gap-3 rounded-xl p-2 text-left transition-colors hover:bg-kumo-tint/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kumo-brand"
          >
            {heading}
            <span className="mt-1 flex shrink-0 items-center gap-2 text-sm text-kumo-subtle">
              {collapsed ? (
                <span className="rounded-full bg-kumo-warning/15 px-2 py-0.5 text-xs font-medium tabular-nums text-kumo-warning">
                  {issues.length} {issues.length === 1 ? "issue" : "issues"}
                </span>
              ) : null}
              <motion.span
                animate={{ rotate: open ? 180 : 0 }}
                transition={springSnappy}
                className="inline-flex"
              >
                <CaretDownIcon size={18} aria-hidden="true" />
              </motion.span>
              <span className="sr-only">
                {open ? "Hide issues" : "Show issues"}
              </span>
            </span>
          </button>
        )}
        <AnimatePresence initial={false}>
          {open ? (
            <motion.ul
              id="overview-issues"
              key="issues"
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
              className="flex flex-col gap-2 overflow-hidden"
            >
              <li aria-hidden="true" className="h-3" />
              {issues.map((i) => (
                <li
                  key={i.text}
                  className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl bg-kumo-elevated px-4 py-3 text-sm ring-1 ring-kumo-hairline"
                >
                  <span className="min-w-0 flex-1 text-pretty">{i.text}</span>
                  <Button
                    size="xs"
                    variant="secondary"
                    icon={<ArrowRightIcon />}
                    onClick={() => navigate(i.go)}
                  >
                    {i.action}
                  </Button>
                </li>
              ))}
            </motion.ul>
          ) : null}
        </AnimatePresence>
      </div>
    </section>
  );
}

function Launcher() {
  const { data } = useCatalog();
  const installed = data?.apps.filter((a) => a.installed) ?? [];
  return (
    <Section
      title="Your Apps"
      actions={
        <Button
          size="sm"
          variant="ghost"
          icon={<PlusIcon />}
          onClick={() => navigate({ tab: "apps" })}
        >
          Get More
        </Button>
      }
    >
      {installed.length === 0 ? (
        <Card className="flex flex-col items-start gap-3 sm:flex-row sm:items-center">
          <p className="flex-1 text-sm text-kumo-subtle">
            Nothing installed yet. Start with the essentials: a password manager
            and ad blocking.
          </p>
          <Button
            variant="primary"
            size="sm"
            onClick={() => navigate({ tab: "apps" })}
          >
            Browse Apps
          </Button>
        </Card>
      ) : (
        <ul className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-6">
          {installed.map((a, i) => (
            <motion.li
              key={a.id}
              variants={riseIn}
              custom={i}
              initial="hidden"
              animate="show"
            >
              <a
                href={a.url}
                target="_blank"
                rel="noopener noreferrer"
                className="group flex flex-col items-center gap-2 rounded-xl p-3 text-center transition-colors hover:bg-kumo-tint"
              >
                <span className="relative transition-transform group-hover:-translate-y-0.5">
                  <AppLogo
                    src={a.icon}
                    name={a.name}
                    category={a.category}
                    size="lg"
                  />
                  <StatusDot
                    tone={a.running ? "success" : "neutral"}
                    live={a.running}
                    label={a.running ? "Running" : "Stopped"}
                    className="absolute -bottom-0.5 -right-0.5 ring-2 ring-kumo-base"
                  />
                </span>
                <span className="w-full truncate text-xs font-medium">
                  {a.name}
                </span>
              </a>
            </motion.li>
          ))}
        </ul>
      )}
    </Section>
  );
}

function TopConsumers() {
  const { data } = useContainers();
  const top = (data ?? [])
    .filter((c) => c.state === "running" && c.mem_bytes != null)
    .toSorted((a, b) => (b.mem_bytes ?? 0) - (a.mem_bytes ?? 0))
    .slice(0, 6);
  const max = Math.max(1, ...top.map((c) => c.mem_bytes ?? 0));
  return (
    <Section
      title="Using the Most Memory"
      actions={
        <Button
          size="sm"
          variant="ghost"
          icon={<CubeIcon />}
          onClick={() => navigate({ tab: "containers" })}
        >
          All Containers
        </Button>
      }
    >
      <Card className="flex flex-col gap-3">
        {top.length === 0 ? (
          <p className="text-sm text-kumo-subtle">No running containers.</p>
        ) : null}
        {top.map((c) => (
          <button
            key={c.id}
            type="button"
            onClick={() => navigate({ tab: "containers", container: c.name })}
            className="group flex flex-col gap-1 rounded-lg text-left"
          >
            <span className="flex items-baseline justify-between gap-3 text-sm">
              <span className="min-w-0 truncate group-hover:underline">
                {c.name}
              </span>
              <span className="shrink-0 tabular-nums text-kumo-subtle">
                {formatBytes(c.mem_bytes)} ·{" "}
                {formatPercent(c.cpu_percent ?? 0, 1)}
              </span>
            </span>
            <span className="h-1.5 overflow-hidden rounded-full bg-kumo-fill">
              <motion.span
                className="block h-full origin-left rounded-full bg-kumo-brand"
                initial={{ scaleX: 0 }}
                animate={{ scaleX: (c.mem_bytes ?? 0) / max }}
                transition={{ type: "spring", stiffness: 200, damping: 26 }}
              />
            </span>
          </button>
        ))}
      </Card>
    </Section>
  );
}

const DOT = {
  info: "neutral",
  success: "success",
  warning: "warning",
  error: "danger",
} as const;

function Activity() {
  const { data } = useQuery({
    queryKey: ["events-feed"],
    queryFn: () => api<{ events: HearthEvent[] }>("/events?after=0"),
    refetchInterval: 5_000,
  });
  const events = (data?.events ?? []).toReversed().slice(0, 8);
  return (
    <Section
      title="Recent Activity"
      actions={
        <LinkButton
          href="?tab=storage"
          size="sm"
          variant="ghost"
          icon={<HardDrivesIcon />}
        >
          Storage
        </LinkButton>
      }
    >
      <Card>
        {events.length === 0 ? (
          <p className="text-sm text-kumo-subtle">
            Quiet so far. Installs, drive changes and logins show up here.
          </p>
        ) : (
          <ol className="relative flex flex-col gap-4 before:absolute before:bottom-1 before:left-[3px] before:top-1 before:w-px before:bg-kumo-hairline">
            {events.map((e) => (
              <li key={e.seq} className="relative flex gap-3 pl-5">
                <StatusDot
                  tone={DOT[e.level]}
                  label={e.level}
                  className="absolute left-0 top-1.5"
                />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium">{e.title}</p>
                  {e.message ? (
                    <p className="truncate text-xs text-kumo-subtle">
                      {e.message}
                    </p>
                  ) : null}
                </div>
                <time
                  className="shrink-0 text-xs tabular-nums text-kumo-subtle"
                  dateTime={new Date(e.at * 1000).toISOString()}
                >
                  {formatRelative(e.at)}
                </time>
              </li>
            ))}
          </ol>
        )}
      </Card>
    </Section>
  );
}
