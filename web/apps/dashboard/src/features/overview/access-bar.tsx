import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { ClipboardText } from "@cloudflare/kumo/components/clipboard-text";
import { Popover } from "@cloudflare/kumo/components/popover";
import {
  ArrowSquareOutIcon,
  CheckCircleIcon,
  CircleIcon,
  DesktopIcon,
  GlobeHemisphereWestIcon,
  DownloadSimpleIcon,
  QrCodeIcon,
  SealCheckIcon,
  ShieldCheckIcon,
  type Icon,
} from "@phosphor-icons/react";
import { motion } from "motion/react";
import { riseIn } from "@hearth/ui/motion";
import { useAccess, useSetup, type AccessLink } from "../../api/hooks";
import { navigate } from "../../lib/url-state";

const KIND: Record<AccessLink["kind"], { label: string; icon: Icon }> = {
  tailscale: { label: "Tailscale", icon: ShieldCheckIcon },
  domain: { label: "Your domain", icon: GlobeHemisphereWestIcon },
  netbird: { label: "NetBird", icon: ShieldCheckIcon },
  local: { label: "This Mac", icon: DesktopIcon },
};

const bare = (url: string) => url.replace(/^https?:\/\//, "");

/** "Where is my server?": the address to open hearth from your other devices, with copy, open and QR. */
/** What remote access needs, in order, and where to fix each part. */
const NEEDED: {
  id: string;
  title: string;
  why: string;
  go?: Record<string, string>;
  href?: string;
  /** A second way to fix it. */
  alt?: { label: string; go: Record<string, string> };
}[] = [
  {
    id: "tailscale",
    title: "Connect Tailscale",
    why: "Your private network between this Mac and your devices.",
    go: { tab: "settings", section: "network" },
  },
  {
    id: "https",
    title: "Turn on Tailscale HTTPS certificates",
    why: "In the Tailscale admin console, enable MagicDNS and HTTPS Certificates. Can’t (a work tailnet)? Use hearth’s own certificate.",
    href: "https://login.tailscale.com/admin/dns",
    alt: { label: "Use hearth’s", go: { tab: "settings", section: "network" } },
  },
  {
    id: "login",
    title: "Add a user",
    why: "What you sign in with from your phone.",
    go: { tab: "settings", section: "users" },
  },
  {
    id: "gateway",
    title: "Turn on the gateway",
    why: "Publishes hearth and your apps behind that login.",
    go: { tab: "settings", section: "access" },
  },
];

function RemoteChecklist() {
  const { data } = useSetup();
  if (!data) return null;
  const done = (id: string) => data.find((c) => c.id === id)?.state === "done";
  const next = NEEDED.find((n) => !done(n.id))?.id;
  return (
    <div
      className="flex flex-col gap-2 border-t border-kumo-hairline pt-3"
      data-tour="remote-access"
    >
      <p className="text-xs font-medium">
        To open hearth from your phone or laptop:
      </p>
      <ol className="flex flex-col gap-2">
        {NEEDED.map((n, i) => {
          const ok = done(n.id);
          const variant = n.id === next ? "primary" : "secondary";
          return (
            <li key={n.id} className="flex items-center gap-3">
              {ok ? (
                <CheckCircleIcon
                  size={18}
                  weight="fill"
                  className="shrink-0 text-kumo-success"
                  aria-hidden="true"
                />
              ) : (
                <CircleIcon
                  size={18}
                  className="shrink-0 text-kumo-subtle"
                  aria-hidden="true"
                />
              )}
              <span className="min-w-0 flex-1">
                <span
                  className={`block text-sm ${ok ? "text-kumo-subtle" : ""}`}
                >
                  {i + 1}. {n.title}
                </span>
                {!ok ? (
                  <span className="block text-xs text-kumo-subtle text-pretty">
                    {n.why}
                  </span>
                ) : null}
              </span>
              {ok ? null : n.href ? (
                <span className="flex shrink-0 flex-wrap justify-end gap-1.5">
                  {n.alt ? (
                    <Button
                      size="xs"
                      variant="secondary"
                      onClick={() => n.alt && navigate(n.alt.go)}
                    >
                      {n.alt.label}
                    </Button>
                  ) : null}
                  <LinkButton
                    href={n.href}
                    external
                    size="xs"
                    variant={variant}
                    icon={<ArrowSquareOutIcon />}
                  >
                    Open
                  </LinkButton>
                </span>
              ) : (
                <Button
                  size="xs"
                  variant={variant}
                  onClick={() => n.go && navigate(n.go)}
                >
                  Fix
                </Button>
              )}
            </li>
          );
        })}
      </ol>
    </div>
  );
}

function QrPopover({
  svg,
  url,
  label,
  title,
  description,
}: {
  svg: string;
  url: string;
  label: string;
  title: string;
  description: string;
}) {
  return (
    <Popover.Root>
      <Popover.Trigger
        render={
          <Button size="sm" variant="secondary" icon={<QrCodeIcon />}>
            {label}
          </Button>
        }
      />
      <Popover.Portal>
        <Popover.Positioner>
          <Popover.Popup className="w-60">
            <Popover.Title>{title}</Popover.Title>
            <Popover.Description>{description}</Popover.Description>
            <div
              className="mt-3 rounded-lg bg-white p-2 [&_svg]:h-auto [&_svg]:w-full"
              role="img"
              aria-label={`QR code for ${url}`}
              // Rendered server-side from our own URL; contains no user markup.
              dangerouslySetInnerHTML={{ __html: svg }}
            />
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

const TRUST_STEPS: [string, string][] = [
  [
    "iPhone, iPad",
    "Scan the code and allow the download. Then Settings → Profile Downloaded → Install, and Settings → General → About → Certificate Trust Settings → turn on “Caddy Local Authority”.",
  ],
  [
    "Android",
    "Download it, then Settings → Security → Encryption & credentials → Install a certificate → CA certificate.",
  ],
  [
    "Mac",
    "Open the file to add it to Keychain Access, double-click “Caddy Local Authority”, and set Trust to Always Trust.",
  ],
  [
    "Windows",
    "Open the file → Install Certificate → Local Machine → Trusted Root Certification Authorities.",
  ],
];

/** hearth's own certificate: each device trusts it once, or browsers warn. */
function TrustStep({ url, qr }: { url: string; qr: string | null }) {
  return (
    <div className="flex flex-col gap-3 border-t border-kumo-hairline pt-3 sm:flex-row sm:items-center sm:gap-4">
      <span className="flex min-w-0 flex-1 items-start gap-3">
        <SealCheckIcon
          size={20}
          weight="duotone"
          className="mt-0.5 shrink-0 text-kumo-brand"
          aria-hidden="true"
        />
        <span className="min-w-0">
          <span className="block text-sm font-medium">
            First, install hearth’s certificate on each device
          </span>
          <span className="block text-xs text-kumo-subtle text-pretty">
            Your tailnet doesn’t issue certificates, so hearth uses its own.
            Trust it once per phone or laptop, or browsers will show a warning.
            Download it at <span className="font-mono">{bare(url)}</span> from
            any device on your tailnet.
          </span>
        </span>
      </span>
      <span className="flex flex-wrap items-center gap-2">
        {qr ? (
          <QrPopover
            svg={qr}
            url={url}
            label="Install on Phone"
            title="Install on your phone"
            description="It must be on Tailscale. Then follow the steps under How."
          />
        ) : null}
        <LinkButton
          href="/api/gateway/ca.crt"
          size="sm"
          variant="secondary"
          icon={<DownloadSimpleIcon />}
        >
          This Mac
        </LinkButton>
        <Popover.Root>
          <Popover.Trigger
            render={
              <Button size="sm" variant="secondary">
                How?
              </Button>
            }
          />
          <Popover.Portal>
            <Popover.Positioner>
              <Popover.Popup className="w-80 max-w-[calc(100vw-2rem)]">
                <Popover.Title>Trust hearth’s certificate</Popover.Title>
                <dl className="mt-2 flex flex-col gap-2 text-xs">
                  {TRUST_STEPS.map(([os, how]) => (
                    <div key={os}>
                      <dt className="font-medium">{os}</dt>
                      <dd className="text-kumo-subtle text-pretty">{how}</dd>
                    </div>
                  ))}
                </dl>
              </Popover.Popup>
            </Popover.Positioner>
          </Popover.Portal>
        </Popover.Root>
      </span>
    </div>
  );
}

export function AccessBar() {
  const { data } = useAccess();
  if (!data) return null;
  const [primary, ...others] = data.links;
  if (!primary) return null;
  const remote = primary.kind !== "local";
  const P = KIND[primary.kind].icon;

  return (
    <motion.section
      aria-label="Server address"
      data-tour="access"
      variants={riseIn}
      initial="hidden"
      animate="show"
      className="flex flex-col gap-3 rounded-xl bg-kumo-base p-4 ring-1 ring-kumo-hairline"
    >
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:gap-4">
        <span className="flex min-w-0 flex-1 items-center gap-3">
          <span
            className={`grid size-10 shrink-0 place-items-center rounded-lg ${remote ? "bg-kumo-tint text-kumo-brand" : "bg-kumo-tint text-kumo-subtle"}`}
          >
            <P size={20} weight="duotone" aria-hidden="true" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-xs text-kumo-subtle">
              {remote
                ? `Your server’s address · ${primary.reach}`
                : "Only reachable on this Mac for now"}
            </span>
            <ClipboardText
              text={bare(primary.url)}
              textToCopy={primary.url}
              size="sm"
              className="mt-0.5 w-fit max-w-full"
            />
          </span>
        </span>

        <span className="flex flex-wrap items-center gap-2">
          {remote ? (
            <>
              {data.qr_svg ? (
                <QrPopover
                  svg={data.qr_svg}
                  url={primary.url}
                  label="Scan on Phone"
                  title="Scan with your phone"
                  description={`It must be on ${KIND[primary.kind].label} to open.`}
                />
              ) : null}
              <LinkButton
                href={primary.url}
                external
                size="sm"
                variant="primary"
                icon={<ArrowSquareOutIcon />}
              >
                Open
              </LinkButton>
            </>
          ) : null}
        </span>
      </div>

      {!remote ? <RemoteChecklist /> : null}
      {primary.trust_url ? (
        <TrustStep url={primary.trust_url} qr={data.trust_qr_svg} />
      ) : null}

      {others.length ? (
        <ul
          className="flex flex-wrap gap-x-5 gap-y-1 border-t border-kumo-hairline pt-3 text-xs text-kumo-subtle"
          aria-label="Other addresses"
        >
          {others.map((l) => (
            <li key={l.kind} className="min-w-0 break-all">
              {KIND[l.kind].label}:{" "}
              <span className="font-mono">{bare(l.url)}</span>
            </li>
          ))}
        </ul>
      ) : null}
    </motion.section>
  );
}
