import { Badge } from "@cloudflare/kumo/components/badge";
import { Button } from "@cloudflare/kumo/components/button";
import { Tooltip } from "@cloudflare/kumo/components/tooltip";
import { PawPrintIcon } from "@phosphor-icons/react";
import { PixelSprite, PETS } from "@hearth/ui/pets";
import { ThemeToggle } from "@hearth/ui/theme";
import { useTailscale } from "../api/hooks";
import { SearchButton } from "./command-palette";

interface HeaderProps {
  name: string;
  petsOn: boolean;
  onTogglePets: () => void;
}

export function Header({ name, petsOn, onTogglePets }: HeaderProps) {
  const ts = useTailscale();
  const connected = ts.data?.backend_state === "Running";
  return (
    <header className="flex items-center gap-3">
      <a href="/" className="flex items-center gap-2 rounded-md" aria-label="hearth home">
        <PixelSprite sprite={PETS.ember.open} scale={2} />
        <span className="text-lg font-semibold tracking-tight" translate="no">
          {name}
        </span>
      </a>
      <div className="ml-auto flex min-w-0 items-center gap-1.5">
        <SearchButton />
        {ts.data ? (
          <Tooltip content={ts.data.dns_name ?? "No MagicDNS name"}>
            <Badge variant={connected ? "success" : "warning"} appearance="dot">
              <span translate="no">Tailscale</span>
              <span className="hidden sm:inline"> {connected ? "connected" : ts.data.backend_state}</span>
            </Badge>
          </Tooltip>
        ) : null}
        <Button
          variant="ghost"
          shape="square"
          onClick={onTogglePets}
          aria-pressed={petsOn}
          aria-label={petsOn ? "Hide pets" : "Show pets"}
          title={petsOn ? "Hide pets" : "Show pets"}
        >
          <PawPrintIcon size={18} weight={petsOn ? "fill" : "regular"} aria-hidden="true" />
        </Button>
        <ThemeToggle />
      </div>
    </header>
  );
}
