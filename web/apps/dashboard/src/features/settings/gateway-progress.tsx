import { AnimatePresence, motion } from "motion/react";
import { Button } from "@cloudflare/kumo/components/button";
import { Loader } from "@cloudflare/kumo/components/loader";
import { ArrowsClockwiseIcon, CheckCircleIcon, CircleIcon, WarningCircleIcon, XIcon } from "@phosphor-icons/react";
import type { GatewayJob } from "../../api/types";

/** Live, step-by-step progress of turning the gateway on. */
export function GatewayProgress({ job, onRetry, onDismiss, retrying }: { job: GatewayJob; onRetry: () => void; onDismiss: () => void; retrying: boolean }) {
  const running = job.finished == null;
  const failed = !running && !job.ok;
  const done = job.steps.filter((s) => s.state === "done").length;
  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      className="overflow-hidden"
    >
      <div className={`my-3 flex flex-col gap-3 rounded-xl p-4 ring-1 ${failed ? "ring-kumo-danger/40" : "ring-kumo-hairline"}`}>
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm font-medium" aria-live="polite">
            {running ? "Setting up the gateway…" : failed ? "The gateway couldn’t start" : "The gateway is live"}
          </p>
          {running ? (
            <span className="text-xs tabular-nums text-kumo-subtle">
              {done} of {job.steps.length}
            </span>
          ) : (
            <Button size="xs" variant="ghost" shape="square" icon={<XIcon />} aria-label="Dismiss" onClick={onDismiss} />
          )}
        </div>
        {running ? (
          <span className="h-1 overflow-hidden rounded-full bg-kumo-tint" aria-hidden="true">
            <motion.span className="block h-full bg-kumo-brand" animate={{ width: `${Math.max(8, (done / job.steps.length) * 100)}%` }} transition={{ duration: 0.4 }} />
          </span>
        ) : null}
        <ol className="flex flex-col gap-2.5">
          {job.steps.map((s) => (
            <li key={s.label} className="flex flex-col gap-0.5">
              <span className="flex items-center gap-2.5 text-sm">
                {s.state === "done" ? (
                  <CheckCircleIcon size={18} weight="fill" className="shrink-0 text-kumo-success" aria-hidden="true" />
                ) : s.state === "failed" ? (
                  <WarningCircleIcon size={18} weight="fill" className="shrink-0 text-kumo-danger" aria-hidden="true" />
                ) : s.state === "running" ? (
                  <Loader size={18} />
                ) : (
                  <CircleIcon size={18} className="shrink-0 text-kumo-subtle" aria-hidden="true" />
                )}
                <span className={s.state === "pending" ? "text-kumo-subtle" : ""}>{s.label}</span>
              </span>
              <AnimatePresence initial={false}>
                {s.detail && s.state !== "pending" ? (
                  <motion.span
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    className={`ml-7 text-xs text-pretty break-words ${s.state === "failed" ? "text-kumo-danger" : "text-kumo-subtle"}`}
                    role={s.state === "failed" ? "alert" : undefined}
                  >
                    {s.detail}
                  </motion.span>
                ) : null}
              </AnimatePresence>
            </li>
          ))}
        </ol>
        {failed ? (
          <div className="flex flex-wrap items-center gap-2">
            <Button size="sm" variant="primary" icon={<ArrowsClockwiseIcon />} loading={retrying} onClick={onRetry}>
              Try Again
            </Button>
            {job.steps.some((s) => s.state === "failed" && /didn't finish within|tailscale serve/i.test(s.detail ?? "")) ? (
              <span className="text-xs text-kumo-subtle text-pretty">
                Publishing needs MagicDNS and HTTPS Certificates turned on for your tailnet (login.tailscale.com/admin/dns).
              </span>
            ) : null}
          </div>
        ) : null}
      </div>
    </motion.div>
  );
}
