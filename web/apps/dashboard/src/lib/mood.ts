import { useEffect, useRef, useState } from "react";
import { useIsMutating, useMutationState } from "@tanstack/react-query";
import type { PetMood } from "@hearth/ui/pets";
import type { StorageResponse, SystemSample } from "../api/types";

/** Pure: how should the pets feel about this machine right now? */
export function moodFromSystem(s: SystemSample | null | undefined): PetMood {
  if (!s) return "idle";
  const memFreePct = (s.mem_available / s.mem_total) * 100;
  const swapPct = s.swap_total > 0 ? (s.swap_used / s.swap_total) * 100 : 0;
  const rootDisk = s.disks.find((d) => d.mount === "/");
  const diskFreePct = rootDisk ? (rootDisk.free / rootDisk.total) * 100 : 100;
  if (memFreePct < 10 || swapPct > 60 || diskFreePct < 5) return "worried";
  if (s.cpu_percent > 70) return "busy";
  if (s.cpu_percent < 8) return "sleepy";
  return "idle";
}

/**
 * Combine system mood with app activity: an install in flight → working,
 * a success in the last few seconds → happy.
 */
export function usePetMood(sample: SystemSample | null | undefined, storage?: StorageResponse): PetMood {
  const busy = useIsMutating();
  const successes = useMutationState({
    filters: { status: "success" },
    select: (m) => m.state.submittedAt,
  }).length;
  const [happy, setHappy] = useState(false);
  const seen = useRef(successes);

  useEffect(() => {
    if (successes <= seen.current) {
      seen.current = successes; // cache entries can be garbage-collected
      return;
    }
    seen.current = successes;
    setHappy(true);
    const t = setTimeout(() => setHappy(false), 4_000);
    return () => clearTimeout(t);
  }, [successes]);

  if (busy > 0) return "working";
  if (happy) return "happy";
  if (storage?.attachments.some((a) => a.status.state === "offline")) return "worried";
  return moodFromSystem(sample);
}
