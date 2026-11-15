import { useEffect, useRef } from "react";
import { createAgentPetAPI, type AgentPetAPI, type WidgetState } from "agent-pet/widget";
import { emberAtlas, type PetMood, type Speech } from "@hearth/ui/pets";

/** hearth mood → agent-pet persistent state. */
const STATE: Record<PetMood, WidgetState> = {
  sleepy: "idle",
  idle: "idle",
  busy: "thinking",
  worried: "waiting",
  working: "building",
  happy: "success",
};

/**
 * Ember as a draggable desktop-style companion, powered by agent-pet
 * (github.com/gibbon/agent-pet, Apache-2.0) with our own runtime-rendered sprite.
 */
export function useCompanion(enabled: boolean, mood: PetMood, speech: Speech | null) {
  const pet = useRef<AgentPetAPI | null>(null);

  useEffect(() => {
    if (!enabled) return;
    const api = createAgentPetAPI();
    const { url, atlas } = emberAtlas();
    const config = { name: "Ember", imageUrl: url, atlas, accent: "#f97316", storageKey: "hearth:companion:v1" };
    api.mount(config);
    // agent-pet 0.8.6 emits its "config changed" event during mount() before it subscribes,
    // so the first config is never applied. Configuring again once mounted fixes it.
    api.configure(config);
    api.play("greeting");
    pet.current = api;
    return () => {
      api.unmount();
      pet.current = null;
    };
  }, [enabled]);

  useEffect(() => {
    const api = pet.current;
    if (!api) return;
    if (mood === "happy") api.play("success", { loops: 2 });
    else api.setState(STATE[mood]);
  }, [mood, enabled]);

  useEffect(() => {
    const api = pet.current;
    if (!api || !speech) return;
    api.say(speech.text, { ttl: 5000 });
    if (/uh oh|not our drive|where did/i.test(speech.text)) api.play("error");
  }, [speech]);
}
