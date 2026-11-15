import { Badge } from "@cloudflare/kumo/components/badge";
import type { Fit } from "../../api/types";

const VARIANT = { ok: "success", warn: "warning", deny: "error" } as const;
const LABEL = { ok: "Fits", warn: "Tight", deny: "Won’t Fit" } as const;

export function FitBadge({ fit }: { fit: Fit }) {
  return (
    <Badge variant={VARIANT[fit]} appearance="dot">
      {LABEL[fit]}
    </Badge>
  );
}
