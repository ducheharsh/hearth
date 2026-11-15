import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { ClipboardText } from "@cloudflare/kumo/components/clipboard-text";
import { Popover } from "@cloudflare/kumo/components/popover";
import {
  ArrowSquareOutIcon,
  DesktopIcon,
  GlobeHemisphereWestIcon,
  LockKeyIcon,
  QrCodeIcon,
  ShieldCheckIcon,
  type Icon,
} from "@phosphor-icons/react";
import { motion } from "motion/react";
import { riseIn } from "@hearth/ui/motion";
import { useAccess, type AccessLink } from "../../api/hooks";
import { navigate } from "../../lib/url-state";

const KIND: Record<AccessLink["kind"], { label: string; icon: Icon }> = {
  tailscale: { label: "Tailscale", icon: ShieldCheckIcon },
  domain: { label: "Your domain", icon: GlobeHemisphereWestIcon },
  netbird: { label: "NetBird", icon: ShieldCheckIcon },
  local: { label: "This Mac", icon: DesktopIcon },
};

const bare = (url: string) => url.replace(/^https?:\/\//, "");

/** "Where is my server?": the address to open hearth from your other devices, with copy, open and QR. */
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
                : "Only reachable on this Mac. Set up access to open hearth from your phone."}
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
                <Popover.Root>
                  <Popover.Trigger
                    render={
                      <Button
                        size="sm"
                        variant="secondary"
                        icon={<QrCodeIcon />}
                      >
                        Scan on Phone
                      </Button>
                    }
                  />
                  <Popover.Portal>
                    <Popover.Positioner>
                      <Popover.Popup className="w-60">
                        <Popover.Title>Scan with your phone</Popover.Title>
                        <Popover.Description>
                          It must be on {KIND[primary.kind].label} to open.
                        </Popover.Description>
                        <div
                          className="mt-3 rounded-lg bg-white p-2 [&_svg]:h-auto [&_svg]:w-full"
                          role="img"
                          aria-label={`QR code for ${primary.url}`}
                          // Rendered server-side from our own URL; contains no user markup.
                          dangerouslySetInnerHTML={{ __html: data.qr_svg }}
                        />
                      </Popover.Popup>
                    </Popover.Positioner>
                  </Popover.Portal>
                </Popover.Root>
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
          ) : (
            <Button
              size="sm"
              variant="primary"
              icon={<LockKeyIcon />}
              onClick={() =>
                navigate({
                  tab: "settings",
                  section: data.gateway_enabled ? "network" : "setup",
                })
              }
            >
              {data.gateway_enabled ? "Connect a Network" : "Set Up Access"}
            </Button>
          )}
        </span>
      </div>

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
