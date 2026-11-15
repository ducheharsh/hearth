import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Empty } from "@cloudflare/kumo/components/empty";
import { Switch } from "@cloudflare/kumo/components/switch";
import { Table } from "@cloudflare/kumo/components/table";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import {
  ArrowClockwiseIcon,
  CubeIcon,
  GlobeHemisphereWestIcon,
  PauseIcon,
  PlayIcon,
  StopIcon,
  TrashIcon,
  WarningIcon,
  XIcon,
} from "@phosphor-icons/react";
import { Card } from "@hearth/ui/components/card";
import { Field, TextInput } from "@hearth/ui/components/form";
import { Section } from "@hearth/ui/components/section";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { formatBytes, formatPercent, formatRelative } from "@hearth/ui/format";
import { useAdopt, useContainer, useContainerAction, useContainers, useRoutes, useUnadopt } from "../../api/hooks";
import type { ContainerInfo, LogLine } from "../../api/types";
import { useUrlParam } from "../../lib/url-state";

// xterm.js is ~300 KB: only load it when a terminal is opened.
const ContainerTerminal = lazy(() => import("./terminal"));

const TONE = { running: "success", restarting: "warning", dead: "danger", paused: "warning" } as const;

export function Containers() {
  const { data, error, isPending } = useContainers();
  const [open, setOpen] = useUrlParam<string>("container", "");
  const routes = useRoutes();

  return (
    <div className="flex flex-col gap-10">
      <Section title="Containers" description="Everything Docker is running. Click a row for details, logs and controls.">
        {error ? (
          <Banner variant="error" icon={<WarningIcon />} title="Docker isn’t reachable" description={`${error.message}. Start OrbStack, then this view updates automatically.`} />
        ) : !isPending && (data?.length ?? 0) === 0 ? (
          <Empty icon={<CubeIcon size={40} />} title="No containers" description="Install an app to see it here." />
        ) : (
          <Card className="overflow-x-auto p-0">
            <Table>
              <Table.Header>
                <Table.Row>
                  <Table.Head>Name</Table.Head>
                  <Table.Head>Project</Table.Head>
                  <Table.Head className="text-right">Memory</Table.Head>
                  <Table.Head className="text-right">CPU</Table.Head>
                  <Table.Head>Image</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {(data ?? []).map((c) => (
                  <Row key={c.id} c={c} onOpen={() => setOpen(c.name)} />
                ))}
              </Table.Body>
            </Table>
          </Card>
        )}
      </Section>

      {(routes.data?.length ?? 0) > 0 ? (
        <Section title="Your Published Containers" description="Containers you run yourself, reachable through the gateway.">
          <Card className="py-1">
            <ul className="divide-y divide-kumo-hairline">
              {routes.data?.map((r) => (
                <PublishedRow key={r.id} id={r.id} name={r.name} detail={`${r.container}:${r.container_port} → :${r.port}${r.protect ? " · login" : ""}`} />
              ))}
            </ul>
          </Card>
        </Section>
      ) : null}

      {open ? <ContainerSheet name={open} onClose={() => setOpen("")} /> : null}
    </div>
  );
}

function PublishedRow({ id, name, detail }: { id: string; name: string; detail: string }) {
  const unadopt = useUnadopt();
  return (
    <li className="flex items-center gap-3 py-2">
      <GlobeHemisphereWestIcon size={18} aria-hidden="true" className="text-kumo-subtle" />
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-medium">{name}</span>
        <span className="block truncate font-mono text-xs text-kumo-subtle">{detail}</span>
      </span>
      <Button size="xs" variant="secondary-destructive" icon={<TrashIcon />} loading={unadopt.isPending} onClick={() => unadopt.mutate(id)}>
        Unpublish
      </Button>
    </li>
  );
}

function Row({ c, onOpen }: { c: ContainerInfo; onOpen: () => void }) {
  const tone = TONE[c.state as keyof typeof TONE] ?? "neutral";
  return (
    <Table.Row className="cursor-pointer hover:bg-kumo-tint" onClick={onOpen}>
      <Table.Cell>
        <button type="button" onClick={onOpen} className="flex items-center gap-2 rounded text-left">
          <StatusDot tone={tone} live={c.state === "running"} label={c.status} />
          <span className="max-w-56 truncate font-medium" title={c.name}>
            {c.name}
          </span>
        </button>
      </Table.Cell>
      <Table.Cell className="text-kumo-subtle">{c.project ?? "–"}</Table.Cell>
      <Table.Cell className="text-right tabular-nums">{formatBytes(c.mem_bytes)}</Table.Cell>
      <Table.Cell className="text-right tabular-nums">{c.cpu_percent == null ? "–" : formatPercent(c.cpu_percent, 1)}</Table.Cell>
      <Table.Cell>
        <span className="block max-w-72 truncate font-mono text-xs text-kumo-subtle" title={c.image} translate="no">
          {c.image}
        </span>
      </Table.Cell>
    </Table.Row>
  );
}

type SheetTab = "logs" | "terminal" | "details" | "publish";

function ContainerSheet({ name, onClose }: { name: string; onClose: () => void }) {
  const { data: d, error } = useContainer(name);
  const act = useContainerAction();
  const [tab, setTab] = useState<SheetTab>("logs");

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="xl" className="flex h-[min(85dvh,52rem)] flex-col gap-4 overflow-hidden p-6">
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <Dialog.Title className="flex items-center gap-2 text-lg font-semibold">
              {d ? <StatusDot tone={d.running ? "success" : "neutral"} live={d.running} label={d.status} /> : null}
              <span className="truncate">{name}</span>
            </Dialog.Title>
            <Dialog.Description className="truncate font-mono text-xs text-kumo-subtle" translate="no">
              {d ? `${d.image} · ${d.status}${d.health ? ` · ${d.health}` : ""} · started ${formatRelative(d.started_at)}` : error ? error.message : "Loading…"}
            </Dialog.Description>
          </div>
          <div className="flex gap-1">
            {d?.running ? (
              <>
                <Button size="sm" icon={<ArrowClockwiseIcon />} loading={act.isPending} onClick={() => act.mutate({ id: name, action: "restart" })}>
                  Restart
                </Button>
                <Button size="sm" icon={<StopIcon />} loading={act.isPending} onClick={() => act.mutate({ id: name, action: "stop" })}>
                  Stop
                </Button>
              </>
            ) : (
              <Button size="sm" variant="primary" icon={<PlayIcon />} loading={act.isPending} disabled={!d} onClick={() => act.mutate({ id: name, action: "start" })}>
                Start
              </Button>
            )}
            <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
          </div>
        </div>
        {act.error ? <Banner variant="error" title="Action failed" description={act.error.message} /> : null}
        <Tabs
          variant="underline"
          value={tab}
          onValueChange={(v) => setTab(v as SheetTab)}
          tabs={[
            { value: "logs", label: "Logs" },
            { value: "terminal", label: "Terminal" },
            { value: "details", label: "Details" },
            { value: "publish", label: "Publish" },
          ]}
        />
        <div className="min-h-0 flex-1">
          {tab === "logs" ? (
            <Logs id={name} />
          ) : tab === "terminal" ? (
            d?.running ? (
              <Suspense fallback={<p className="text-sm text-kumo-subtle">Loading terminal…</p>}>
                <ContainerTerminal id={name} />
              </Suspense>
            ) : (
              <p className="text-sm text-kumo-subtle">Start the container to open a terminal.</p>
            )
          ) : tab === "publish" ? <Publish container={name} project={d?.project ?? null} /> : d ? <Details d={d} /> : null}
        </div>
      </Dialog>
    </Dialog.Root>
  );
}

/** Live logs over Server-Sent Events, with follow/pause. */
function Logs({ id }: { id: string }) {
  const [lines, setLines] = useState<LogLine[]>([]);
  const [follow, setFollow] = useState(true);
  const [paused, setPaused] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    setLines([]);
    const es = new EventSource(`/api/containers/${encodeURIComponent(id)}/logs/stream?tail=300`);
    es.onmessage = (ev) => {
      if (pausedRef.current) return;
      const line = JSON.parse(ev.data) as LogLine;
      setLines((prev) => (prev.length > 2000 ? [...prev.slice(-1500), line] : [...prev, line]));
    };
    return () => es.close();
  }, [id]);

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines, follow]);

  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <Switch label="Follow" checked={follow} onCheckedChange={setFollow} size="sm" />
        <div className="flex gap-1">
          <Button size="xs" variant="ghost" icon={paused ? <PlayIcon /> : <PauseIcon />} onClick={() => setPaused((p) => !p)}>
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button size="xs" variant="ghost" onClick={() => setLines([])}>
            Clear
          </Button>
        </div>
      </div>
      <div
        ref={box}
        role="log"
        aria-live="off"
        aria-label="Container logs"
        className="min-h-0 flex-1 overflow-auto overscroll-contain rounded-lg bg-kumo-recessed p-3 font-mono text-xs leading-relaxed"
      >
        {lines.length === 0 ? <p className="text-kumo-subtle">Waiting for output…</p> : null}
        {lines.map((l, i) => (
          <div key={i} className={l.stream === "stderr" ? "whitespace-pre-wrap break-all text-kumo-warning" : "whitespace-pre-wrap break-all"}>
            {l.text}
          </div>
        ))}
      </div>
    </div>
  );
}

function Details({ d }: { d: NonNullable<ReturnType<typeof useContainer>["data"]> }) {
  const rows: [string, string][] = [
    ["Image", d.image],
    ["Status", `${d.status}${d.exit_code != null && !d.running ? ` (exit ${d.exit_code})` : ""}`],
    ["Health", d.health ?? "–"],
    ["Restart policy", d.restart_policy ?? "–"],
    ["Restarts", String(d.restart_count ?? 0)],
    ["Compose project", d.project ? `${d.project} / ${d.service ?? ""}` : "–"],
    ["Networks", d.networks.join(", ") || "–"],
    ["Created", formatRelative(d.created)],
  ];
  return (
    <div className="flex h-full flex-col gap-5 overflow-auto overscroll-contain pr-1">
      <dl className="grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
        {rows.map(([k, v]) => (
          <div key={k} className="flex min-w-0 justify-between gap-3 border-b border-kumo-hairline py-1.5">
            <dt className="text-kumo-subtle">{k}</dt>
            <dd className="truncate text-right font-mono text-xs" title={v}>
              {v}
            </dd>
          </div>
        ))}
      </dl>
      <DetailList title="Ports" empty="No published ports (reached through the gateway)." items={d.ports.map((p) => `${p.container} → ${p.host.join(", ") || "internal"}`)} />
      <DetailList title="Mounts" empty="No mounts." items={d.mounts.map((m) => `${m.source} → ${m.destination}${m.read_only ? " (read-only)" : ""}`)} />
      <DetailList title="Environment" empty="No variables." items={d.env} />
    </div>
  );
}

function DetailList({ title, items, empty }: { title: string; items: string[]; empty: string }) {
  return (
    <section>
      <h3 className="mb-1.5 text-sm font-medium">{title}</h3>
      {items.length === 0 ? (
        <p className="text-xs text-kumo-subtle">{empty}</p>
      ) : (
        <ul className="rounded-lg bg-kumo-recessed p-3 font-mono text-xs leading-relaxed">
          {items.map((x) => (
            <li key={x} className="break-all">
              {x}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** Bring your own container into the gateway (login + tailnet HTTPS + optional domain). */
function Publish({ container, project }: { container: string; project: string | null }) {
  const adopt = useAdopt();
  const [name, setName] = useState(container.replace(/[-_]\d+$/, ""));
  const [cport, setCport] = useState("80");
  const [port, setPort] = useState("4200");
  const [protect, setProtect] = useState(true);
  if (project?.startsWith("hearth-")) {
    return <p className="text-sm text-kumo-subtle">This container is managed by hearth. Change its address from the app’s details in Apps.</p>;
  }
  return (
    <form
      className="flex max-w-xl flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        adopt.mutate({ container, name, container_port: Number(cport), port: Number(port), protect });
      }}
    >
      <p className="text-sm text-kumo-subtle text-pretty">
        Publish this container through hearth: it joins the gateway network and gets an HTTPS address on your tailnet, behind your login.
      </p>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <Field label="Name">{(id) => <TextInput id={id} required value={name} autoComplete="off" onChange={(e) => setName(e.target.value)} />}</Field>
        <Field label="Container port" hint="Inside the container.">
          {(id) => <TextInput id={id} required inputMode="numeric" value={cport} onChange={(e) => setCport(e.target.value.replace(/\D/g, ""))} />}
        </Field>
        <Field label="Tailnet port" hint="https://…ts.net:port">
          {(id) => <TextInput id={id} required inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} />}
        </Field>
      </div>
      <Switch label="Require my login" checked={protect} onCheckedChange={setProtect} />
      {adopt.error ? <Banner variant="error" title="Couldn’t publish" description={adopt.error.message} /> : null}
      {adopt.isSuccess ? <Banner title="Published" description={`Reachable on port ${adopt.data.port} once the gateway is on.`} /> : null}
      <Button type="submit" variant="primary" icon={<GlobeHemisphereWestIcon />} loading={adopt.isPending} className="self-start">
        Publish Container
      </Button>
    </form>
  );
}
