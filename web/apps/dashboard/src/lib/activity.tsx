import { createContext, use, useEffect, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useKumoToastManager } from "@cloudflare/kumo/components/toast";
import type { Speech } from "@hearth/ui/pets";
import { useEvents } from "../api/hooks";
import type { HearthEvent } from "../api/types";

const VARIANT = { info: "info", success: "success", warning: "warning", error: "error" } as const;

/** What the pets say for an event (short, friendly). */
function speechFor(e: HearthEvent): string {
  switch (e.title) {
    case "Drive reconnected":
      return "Our drive is back! 🎉";
    case "Drive disconnected":
      return "Hey… where did the drive go?";
    case "Different drive detected":
      return "That's not our drive!";
    case "New drive detected":
      return "Ooh, a new drive!";
    case "Gateway is live":
      return "We're online!";
    default:
      if (e.title.endsWith(" installed")) return `Yay, ${e.title.replace(" installed", "")} moved in!`;
      if (e.title.endsWith(" removed")) return "Bye bye…";
      return e.level === "error" ? "Uh oh." : e.title;
  }
}

const SpeechContext = createContext<Speech | null>(null);
export const useSpeech = () => use(SpeechContext);

/**
 * Polls the daemon's activity feed: every new event becomes a toast and the pets say
 * something about it. Also refreshes affected queries so the UI reacts immediately.
 */
export function ActivityProvider({ children }: { children: ReactNode }) {
  const [after, setAfter] = useState<number | null>(null);
  const [speech, setSpeech] = useState<Speech | null>(null);
  const { data } = useEvents(after ?? 0);
  const toasts = useKumoToastManager();
  const qc = useQueryClient();
  const seq = useRef(0);

  useEffect(() => {
    if (!data) return;
    if (after === null) {
      // First load: don't replay history.
      setAfter(data.last);
      return;
    }
    if (data.events.length === 0) return;
    for (const e of data.events) {
      toasts.add({ variant: VARIANT[e.level], title: e.title, description: e.message || undefined });
    }
    const last = data.events[data.events.length - 1];
    if (last) {
      seq.current += 1;
      setSpeech({ id: seq.current, text: speechFor(last) });
    }
    const sources = new Set(data.events.map((e) => e.source));
    if (sources.has("storage")) void qc.invalidateQueries({ queryKey: ["storage"] });
    if (sources.has("apps")) void qc.invalidateQueries({ queryKey: ["catalog"] });
    void qc.invalidateQueries({ queryKey: ["setup"] });
    setAfter(data.last);
  }, [data, after, toasts, qc]);

  // Speech bubbles disappear after a few seconds.
  useEffect(() => {
    if (!speech) return;
    const t = setTimeout(() => setSpeech(null), 4_500);
    return () => clearTimeout(t);
  }, [speech]);

  return <SpeechContext value={speech}>{children}</SpeechContext>;
}
