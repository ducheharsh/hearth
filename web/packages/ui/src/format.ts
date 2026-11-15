/** Locale-aware formatting (Intl only, never hand-rolled formats). */

const KIB = 1024;
const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

const nf = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const nf0 = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

/** 1536 → "1.5 KB" (binary units, short labels). */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes)) return "–";
  let v = bytes;
  let i = 0;
  while (v >= KIB && i < UNITS.length - 1) {
    v /= KIB;
    i++;
  }
  return `${i >= 3 ? nf.format(v) : nf0.format(v)} ${UNITS[i]}`;
}

export function formatPercent(n: number | null | undefined, digits = 0): string {
  if (n == null || !Number.isFinite(n)) return "–";
  return new Intl.NumberFormat(undefined, {
    style: "percent",
    maximumFractionDigits: digits,
  }).format(n / 100);
}

export function formatDuration(totalSeconds: number): string {
  const d = Math.floor(totalSeconds / 86400);
  const h = Math.floor((totalSeconds % 86400) / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatNumber(n: number, digits = 2): string {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: digits }).format(n);
}

const timeFmt = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" });
export function formatTime(unixSeconds: number): string {
  return timeFmt.format(new Date(unixSeconds * 1000));
}

const compact = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 });
/** 118657 → "119K" */
export function formatCompact(n: number | null | undefined): string {
  return n == null ? "–" : compact.format(n);
}

const rel = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
/** ISO date or unix seconds → "3 days ago". */
export function formatRelative(when: string | number | null | undefined): string {
  if (when == null) return "–";
  const t = typeof when === "number" ? when * 1000 : Date.parse(when);
  if (Number.isNaN(t)) return "–";
  const secs = Math.round((t - Date.now()) / 1000);
  const abs = Math.abs(secs);
  if (abs < 60) return rel.format(secs, "second");
  if (abs < 3600) return rel.format(Math.round(secs / 60), "minute");
  if (abs < 86400) return rel.format(Math.round(secs / 3600), "hour");
  if (abs < 86400 * 45) return rel.format(Math.round(secs / 86400), "day");
  if (abs < 86400 * 400) return rel.format(Math.round(secs / (86400 * 30)), "month");
  return rel.format(Math.round(secs / (86400 * 365)), "year");
}

const monthFmt = new Intl.DateTimeFormat(undefined, { month: "long" });
/** "2026-11" → "November" */
export function formatMonth(ym: string | null | undefined): string {
  if (!ym) return "this month";
  const [y, m] = ym.split("-").map(Number);
  return y && m ? monthFmt.format(new Date(y, m - 1, 1)) : ym;
}
