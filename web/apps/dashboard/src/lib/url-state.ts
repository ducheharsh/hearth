import { useCallback, useSyncExternalStore } from "react";

/** Query-string state so tabs, search and filters are deep-linkable. */

const listeners = new Set<() => void>();
const notify = () => listeners.forEach((l) => l());

function subscribe(cb: () => void) {
  listeners.add(cb);
  window.addEventListener("popstate", cb);
  return () => {
    listeners.delete(cb);
    window.removeEventListener("popstate", cb);
  };
}

export function useUrlParam<T extends string>(key: string, fallback: T): [T, (v: T) => void] {
  const value = useSyncExternalStore(
    subscribe,
    () => (new URLSearchParams(window.location.search).get(key) as T | null) ?? fallback,
    () => fallback,
  );
  const set = useCallback(
    (v: T) => {
      const params = new URLSearchParams(window.location.search);
      if (v === fallback || v === "") params.delete(key);
      else params.set(key, v);
      const qs = params.toString();
      window.history.replaceState(null, "", qs ? `?${qs}` : window.location.pathname);
      notify();
    },
    [key, fallback],
  );
  return [value, set];
}

/** Replace the whole query string (e.g. jump to `?tab=settings&section=network`). */
export function navigate(params: Record<string, string>) {
  const qs = new URLSearchParams(Object.entries(params).filter(([, v]) => v !== "")).toString();
  window.history.pushState(null, "", qs ? `?${qs}` : window.location.pathname);
  notify();
}
