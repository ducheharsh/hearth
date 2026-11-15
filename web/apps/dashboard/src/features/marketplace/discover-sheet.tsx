import { useState } from "react";
import { motion } from "motion/react";
import { useQuery } from "@tanstack/react-query";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { SkeletonLine } from "@cloudflare/kumo/components/loader";
import { ArrowSquareOutIcon, GithubLogoIcon, GlobeIcon, MonitorPlayIcon, PlusIcon, StarIcon, XIcon } from "@phosphor-icons/react";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { Stars } from "@hearth/ui/components/stars";
import { formatCompact, formatMonth, formatRelative } from "@hearth/ui/format";
import { springSoft } from "@hearth/ui/motion";
import { api } from "../../api/client";
import { useCatalog, useRate } from "../../api/hooks";
import type { DiscoverEntry } from "../../api/types";
import { useUrlParam } from "../../lib/url-state";
import { useAppDialogs } from "./app-card";

interface Detail {
  entry: DiscoverEntry;
  rating: number | null;
  catalog_id: string | null;
  related: { slug: string; name: string; stars: number | null; has_icon: boolean; description: string }[];
  has_preview: boolean;
  directory_url: string;
}

/** Opens the hearth page for a directory app (`?app=<slug>`). */
export function useOpenDiscoverApp() {
  const [, setSlug] = useUrlParam<string>("app", "");
  return setSlug;
}

/** Mounted once; shows whichever app `?app=` points at. */
export function DiscoverSheetHost() {
  const [slug, setSlug] = useUrlParam<string>("app", "");
  return slug ? <DiscoverSheet slug={slug} onClose={() => setSlug("")} onOpen={setSlug} /> : null;
}

function ActivityBars({ commits }: { commits: Record<string, number> }) {
  const months = Object.keys(commits).toSorted().slice(-12);
  const max = Math.max(1, ...months.map((m) => commits[m] ?? 0));
  if (months.length === 0) return <p className="text-xs text-kumo-subtle">No commit history.</p>;
  return (
    <div className="flex h-24 items-end gap-1" role="img" aria-label="Commits per month over the last year">
      {months.map((m, i) => {
        const v = commits[m] ?? 0;
        return (
          <div key={m} className="group flex flex-1 flex-col items-center gap-1">
            <motion.div
              className="w-full origin-bottom rounded-sm bg-kumo-brand/80 group-hover:bg-kumo-brand"
              style={{ height: `${Math.max(4, (v / max) * 100)}%` }}
              initial={{ scaleY: 0 }}
              animate={{ scaleY: 1 }}
              transition={{ ...springSoft, delay: i * 0.03 }}
              title={`${formatMonth(m)}: ${v} commits`}
            />
            <span className="text-[9px] text-kumo-subtle">{formatMonth(m).slice(0, 1)}</span>
          </div>
        );
      })}
    </div>
  );
}

function DiscoverSheet({ slug, onClose, onOpen }: { slug: string; onClose: () => void; onOpen: (s: string) => void }) {
  const { data, error } = useQuery({ queryKey: ["discover-app", slug], queryFn: () => api<Detail>(`/discover/app/${slug}`) });
  const catalog = useCatalog();
  const rate = useRate();
  const [previewOk, setPreviewOk] = useState(true);
  const e = data?.entry;
  const app = data?.catalog_id ? catalog.data?.apps.find((a) => a.id === data.catalog_id) : undefined;

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="xl" className={DIALOG_FRAME}>
        <DialogFrame>
        <div className="flex flex-col gap-6">
        {error ? (
          <Dialog.Title className="text-lg font-semibold">App not found</Dialog.Title>
        ) : !e ? (
          <div className="flex flex-col gap-3">
            <Dialog.Title className="sr-only">Loading app</Dialog.Title>
            <SkeletonLine />
            <SkeletonLine />
          </div>
        ) : (
          <>
            <div className="flex items-start gap-4">
              <AppLogo src={e.has_icon ? `/api/icons/${e.slug}.webp` : null} name={e.name} category="" size="lg" />
              <div className="min-w-0 flex-1">
                <Dialog.Title className="text-xl font-semibold tracking-tight">{e.name}</Dialog.Title>
                <p className="text-sm text-kumo-subtle">{e.tags.join(" · ")}</p>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {e.licenses.map((l) => (
                    <Badge key={l} variant="neutral">
                      {l}
                    </Badge>
                  ))}
                  {e.platforms.map((p) => (
                    <Badge key={p} variant="outline">
                      {p}
                    </Badge>
                  ))}
                  {e.depends_3rdparty ? <Badge variant="warning">Needs a 3rd-party service</Badge> : null}
                </div>
              </div>
              <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
            </div>

            <div className="flex flex-wrap gap-2">
              {app ? <InstallButton appId={app.id} /> : null}
              {e.website ? (
                <LinkButton href={e.website} external variant={app ? "secondary" : "primary"} icon={<GlobeIcon />}>
                  Website
                </LinkButton>
              ) : null}
              {e.demo ? (
                <LinkButton href={e.demo} external icon={<MonitorPlayIcon />}>
                  Live Demo
                </LinkButton>
              ) : null}
              {e.source ? (
                <LinkButton href={e.source} external variant="ghost" icon={<GithubLogoIcon />}>
                  Source
                </LinkButton>
              ) : null}
              <LinkButton href={data.directory_url} external variant="ghost" icon={<ArrowSquareOutIcon />}>
                awesome-selfhosted
              </LinkButton>
            </div>
            {!app ? (
              <p className="-mt-3 text-xs text-kumo-subtle text-pretty">
                Not in the one-click catalog yet. If it ships a Docker image, use <strong>Add Your Own</strong> on the Apps page to install it with hearth’s login and capacity checks.
              </p>
            ) : null}

            <p className="text-pretty">{e.description}</p>

            <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <Stat label="GitHub stars" value={e.stars != null ? `★ ${formatCompact(e.stars)}` : "–"} />
              <Stat label="Latest release" value={e.release?.tag ?? "–"} mono />
              <Stat label="Released" value={formatRelative(e.release?.published_at)} />
              <Stat label="Last updated" value={formatRelative(e.updated_at)} />
            </dl>

            {data.has_preview && previewOk ? (
              <section aria-label="Gallery" className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">Gallery</h3>
                <img
                  src={`/api/discover/app/${e.slug}/preview`}
                  alt={`${e.name} project card`}
                  width={1200}
                  height={600}
                  loading="lazy"
                  onError={() => setPreviewOk(false)}
                  className="h-auto w-full rounded-xl ring-1 ring-kumo-hairline"
                />
              </section>
            ) : null}

            <section aria-label="Development activity" className="flex flex-col gap-2">
              <h3 className="text-sm font-medium">Activity (commits per month)</h3>
              <ActivityBars commits={e.commits} />
            </section>

            <section aria-label="Your rating" className="flex flex-col gap-2">
              <h3 className="text-sm font-medium">Your Rating</h3>
              <Stars value={data.rating} size={22} onChange={(stars) => rate.mutate({ key: e.slug, stars })} label={`Rate ${e.name}`} />
            </section>

            {data.related.length ? (
              <section aria-label="Similar apps" className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">Similar Apps</h3>
                <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                  {data.related.map((r) => (
                    <li key={r.slug}>
                      <button type="button" onClick={() => onOpen(r.slug)} className="flex w-full items-center gap-3 rounded-xl p-2 text-left ring-1 ring-kumo-hairline hover:bg-kumo-tint">
                        <AppLogo src={r.has_icon ? `/api/icons/${r.slug}.webp` : null} name={r.name} category="" size="sm" />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-sm font-medium">{r.name}</span>
                          <span className="block truncate text-xs text-kumo-subtle">{r.description}</span>
                        </span>
                        {r.stars != null ? (
                          <span className="inline-flex items-center gap-0.5 text-xs tabular-nums text-kumo-subtle">
                            <StarIcon size={11} weight="fill" aria-hidden="true" /> {formatCompact(r.stars)}
                          </span>
                        ) : null}
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
          </>
        )}
        </div>
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}

function InstallButton({ appId }: { appId: string }) {
  const catalog = useCatalog();
  const app = catalog.data?.apps.find((a) => a.id === appId);
  if (!app) return null;
  return <InstallInner app={app} />;
}

function InstallInner({ app }: { app: NonNullable<ReturnType<typeof useCatalog>["data"]>["apps"][number] }) {
  const dialogs = useAppDialogs(app);
  return (
    <>
      {app.installed ? (
        <LinkButton href={app.url} external variant="primary" icon={<ArrowSquareOutIcon />}>
          Open
        </LinkButton>
      ) : (
        <Button variant="primary" icon={<PlusIcon />} onClick={() => dialogs.open("install")}>
          Install with hearth
        </Button>
      )}
      {dialogs.node}
    </>
  );
}

function Stat({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-xl bg-kumo-elevated p-3 ring-1 ring-kumo-hairline">
      <dt className="text-xs text-kumo-subtle">{label}</dt>
      <dd className={mono ? "truncate font-mono text-sm" : "truncate text-sm font-semibold tabular-nums"}>{value}</dd>
    </div>
  );
}
