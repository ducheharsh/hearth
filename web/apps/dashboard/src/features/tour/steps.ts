/** The product tour, in order. `tab`/`section` say where the target lives. */
export interface TourStep {
  title: string;
  body: string;
  /** CSS selector; omit for a centred card. */
  el?: string;
  tab?: string;
  section?: string;
  side?: "top" | "bottom" | "left" | "right";
  /** Wait for this (e.g. loaded content) before showing; defaults to `el`. */
  ready?: string;
}

export const STEPS: TourStep[] = [
  {
    title: "Welcome to hearth",
    body: "This Mac is now your home server. Here’s a one-minute tour of where everything lives.",
  },
  {
    tab: "overview",
    el: '[data-tour="access"]',
    title: "Your server’s address",
    body: "Open hearth from your phone with this link or its QR code. Until remote access is set up, the checklist here shows exactly what’s missing.",
  },
  {
    tab: "overview",
    el: '[data-tour="health"]',
    title: "Health at a glance",
    body: "Anything that needs you shows up here: setup steps, updates, drives, memory and disk.",
  },
  {
    tab: "apps",
    el: '[data-tour="packs"]',
    ready: '[data-tour="packs"] button',
    title: "Packs",
    body: "Set up a whole use case in one click, like Local AI, Family Cloud or a Media Server. hearth checks it fits on this Mac first.",
    side: "top",
  },
  {
    tab: "apps",
    el: '[data-tour="store-views"]',
    title: "The App Store",
    body: "Today’s picks, every one-click app, and 1,300+ more to discover. Every install is checked against your memory and disk.",
  },
  {
    el: '[data-tour="nav"]',
    title: "Everything else",
    body: "Containers for logs and a terminal, Storage for your drives, Backups for nightly snapshots and Settings for access and alerts.",
  },
  {
    tab: "backups",
    el: '[data-tour="backups"]',
    ready: '[data-tour="backups"] h1, [data-tour="backups"] h2',
    title: "Backups",
    body: "Turn on nightly, encrypted backups once. Restore any app to an earlier day in one click.",
    side: "bottom",
  },
  {
    tab: "settings",
    section: "setup",
    el: '[aria-label="Settings sections"]',
    title: "Settings",
    body: "Network, logins with two-factor, the gateway, alerts and domains. Setup at the top tells you what’s left.",
    side: "right",
  },
  {
    el: '[data-tour="search"]',
    title: "Search everything",
    body: "Press ⌘K anywhere to jump to an app, container, drive or setting.",
  },
  {
    el: '[data-tour="pets"]',
    title: "Meet Ember",
    body: "Ember keeps you company and reacts to what your server is doing. Show or hide pets here.",
  },
  {
    tab: "overview",
    title: "You’re all set",
    body: "Start with the checklist on Overview. You can replay this tour anytime from the ? button in the header.",
  },
];
