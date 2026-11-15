import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { useSetup, useStorage, useSystem } from "./api/hooks";
import { GlobalSearch } from "./components/command-palette";
import { Header } from "./components/header";
import { Backups } from "./features/backups/backups";
import { Containers } from "./features/containers/containers";
import { DiscoverSheetHost } from "./features/marketplace/discover-sheet";
import { Marketplace } from "./features/marketplace/marketplace";
import { Overview } from "./features/overview/overview";
import { Settings } from "./features/settings/settings";
import { Storage } from "./features/storage/storage";
import { ActivityProvider, useSpeech } from "./lib/activity";
import { useCompanion } from "./lib/companion";
import { usePetMood } from "./lib/mood";
import { useUrlParam } from "./lib/url-state";

type Tab = "overview" | "apps" | "containers" | "storage" | "backups" | "settings";

const PETS_KEY = "hearth:pets:v1";
function readPets(): boolean {
  try {
    return localStorage.getItem(PETS_KEY) !== "off";
  } catch {
    return true;
  }
}

export function App() {
  return (
    <ActivityProvider>
      <Shell />
    </ActivityProvider>
  );
}

function Shell() {
  const [tab, setTab] = useUrlParam<Tab>("tab", "overview");
  const [petsOn, setPetsOn] = useState(readPets);
  const system = useSystem();
  const storage = useStorage();
  const setup = useSetup();
  const mood = usePetMood(system.data?.latest, storage.data);
  const speech = useSpeech();
  useCompanion(petsOn, mood, speech);
  const name = system.data?.display_name ?? "hearth";
  const todo = setup.data?.filter((c) => c.state === "todo").length ?? 0;

  useEffect(() => {
    document.title = name === "hearth" ? "hearth" : `${name} · hearth`;
  }, [name]);

  const togglePets = () =>
    setPetsOn((on) => {
      try {
        localStorage.setItem(PETS_KEY, on ? "off" : "on");
      } catch {
        /* ignore */
      }
      return !on;
    });

  const tabs = [
    { value: "overview", label: "Overview" },
    { value: "apps", label: "Apps" },
    { value: "containers", label: "Containers" },
    { value: "storage", label: "Storage" },
    { value: "backups", label: "Backups" },
    { value: "settings", label: todo > 0 ? `Settings (${todo})` : "Settings" },
  ];

  return (
    <div className="mx-auto flex min-h-dvh max-w-6xl flex-col px-4 pb-16 pt-5 sm:px-6">
      <a href="#main" className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-kumo-base focus:px-3 focus:py-2">
        Skip to content
      </a>
      <Header name={name} petsOn={petsOn} onTogglePets={togglePets} />

      <nav className="mt-4 overflow-x-auto [scrollbar-width:none]" aria-label="Sections">
        <Tabs variant="underline" tabs={tabs} value={tab} onValueChange={(v) => setTab(v as Tab)} />
      </nav>

      <main id="main" className="mt-8 scroll-mt-4">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div key={tab} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={{ duration: 0.18 }}>
            {tab === "apps" ? (
              <Marketplace />
            ) : tab === "containers" ? (
              <Containers />
            ) : tab === "storage" ? (
              <Storage />
            ) : tab === "backups" ? (
              <Backups />
            ) : tab === "settings" ? (
              <Settings />
            ) : (
              <Overview />
            )}
          </motion.div>
        </AnimatePresence>
      </main>
      <DiscoverSheetHost />
      <GlobalSearch />
    </div>
  );
}
