import type { Driver } from "driver.js";
import { navigate } from "../../lib/url-state";
import { STEPS } from "./steps";

const DONE_KEY = "hearth-tour-done";
let active: Driver | null = null;

function remember() {
  try {
    localStorage.setItem(DONE_KEY, "1");
  } catch {
    /* private mode: the tour may show again, that's fine */
  }
}

/** First visit on a desktop-sized screen. */
export function shouldAutoStart(): boolean {
  try {
    return (
      localStorage.getItem(DONE_KEY) == null &&
      window.matchMedia("(min-width: 768px)").matches
    );
  } catch {
    return false;
  }
}

function currentTab(): string {
  return new URLSearchParams(window.location.search).get("tab") ?? "overview";
}

/** Wait until `selector` exists (tabs render after navigation). */
function waitFor(selector: string | undefined, ms = 2500): Promise<void> {
  if (!selector) return Promise.resolve();
  const start = performance.now();
  return new Promise((resolve) => {
    const tick = () => {
      if (document.querySelector(selector) || performance.now() - start > ms)
        resolve();
      else requestAnimationFrame(tick);
    };
    tick();
  });
}

async function go(d: Driver, i: number) {
  const step = STEPS[i];
  if (!step) {
    d.destroy();
    return;
  }
  const moving = Boolean(
    step.tab && (step.tab !== currentTab() || step.section),
  );
  // Hide the previous card while the next page loads, so it never floats over the wrong page.
  if (moving) document.documentElement.classList.add("hearth-tour-moving");
  try {
    if (moving && step.tab)
      navigate(
        step.section
          ? { tab: step.tab, section: step.section }
          : { tab: step.tab },
      );
    await waitFor(step.ready ?? step.el, step.ready ? 6000 : 2500);
    const el = step.el ? document.querySelector(step.el) : null;
    el?.scrollIntoView({
      block: "center",
      behavior: "instant" as ScrollBehavior,
    });
    d.drive(i);
  } finally {
    requestAnimationFrame(() =>
      document.documentElement.classList.remove("hearth-tour-moving"),
    );
  }
}

/** Start (or restart) the product tour. driver.js is loaded only now. */
export async function startTour() {
  const [{ driver }] = await Promise.all([
    import("driver.js"),
    import("driver.js/dist/driver.css"),
    import("./tour.css"),
  ]);
  active?.destroy();
  const d = driver({
    steps: STEPS.map((s) => ({
      element: s.el,
      popover: {
        title: s.title,
        description: s.body,
        side: s.side,
        align: "start",
      },
    })),
    showProgress: true,
    progressText: "{{current}} of {{total}}",
    nextBtnText: "Next",
    prevBtnText: "Back",
    doneBtnText: "Get Started",
    popoverClass: "hearth-tour",
    overlayColor: "#000",
    overlayOpacity: 0.62,
    stagePadding: 8,
    stageRadius: 14,
    popoverOffset: 12,
    smoothScroll: true,
    allowKeyboardControl: true,
    onNextClick: (_el, _step, { driver: dr }) => {
      const i = dr.getActiveIndex() ?? 0;
      if (i >= STEPS.length - 1) dr.destroy();
      else void go(dr, i + 1);
    },
    onPrevClick: (_el, _step, { driver: dr }) =>
      void go(dr, Math.max(0, (dr.getActiveIndex() ?? 1) - 1)),
    onPopoverRender: (popover) => {
      // Ember next to every title, in our pixel style.
      const icon = document.createElement("img");
      icon.src = "/favicon.svg";
      icon.alt = "";
      icon.className = "hearth-tour-ember";
      popover.title.prepend(icon);
    },
    onDestroyed: () => {
      remember();
      active = null;
    },
  });
  active = d;
  await go(d, 0);
}
