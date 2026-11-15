import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useAnimate, useReducedMotion } from "motion/react";
import { cn } from "@cloudflare/kumo/utils";
import { springBouncy } from "../motion";
import { PixelSprite } from "./pixel-sprite";
import { BOX, HEART, PETS, SWEAT, type PetKind } from "./sprites";

/**
 * How the pets feel. Derived by the app from live system state:
 * - sleepy: machine idle          - idle: normal
 * - busy: high CPU                - worried: memory/disk pressure
 * - working: an install/removal is running
 * - happy: something just succeeded
 */
export type PetMood = "sleepy" | "idle" | "busy" | "worried" | "working" | "happy";

const SCALE = 4;

/** Wander cadence (ms) and walking speed (px/s) per mood. `null` = stay put. */
const WANDER: Record<PetMood, { every: number; speed: number } | null> = {
  sleepy: null,
  idle: { every: 5200, speed: 40 },
  busy: { every: 1600, speed: 120 },
  worried: { every: 3000, speed: 70 },
  working: { every: 1200, speed: 140 },
  happy: { every: 2500, speed: 90 },
};

function useWidth<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setWidth(entry?.contentRect.width ?? 0));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, width] as const;
}

interface PetProps {
  kind: PetKind;
  mood: PetMood;
  stageWidth: number;
  start: number;
  /** Speech bubble text (changes trigger a new bubble). */
  say?: Speech | null;
}

export interface Speech {
  id: number;
  text: string;
}

let heartSeq = 0;

function Pet({ kind, mood, stageWidth, start, say }: PetProps) {
  const def = PETS[kind];
  const reduce = useReducedMotion();
  const [scope, animate] = useAnimate<HTMLDivElement>();
  const [blink, setBlink] = useState(false);
  const [facing, setFacing] = useState<1 | -1>(1);
  const [hearts, setHearts] = useState<number[]>([]);
  const x = useRef(start * stageWidth);
  const spriteW = def.open.width * SCALE;

  // Blink every few seconds (eyes closed while sleeping).
  useEffect(() => {
    if (mood === "sleepy") return;
    let t: ReturnType<typeof setTimeout>;
    const loop = () => {
      t = setTimeout(() => {
        setBlink(true);
        t = setTimeout(() => {
          setBlink(false);
          loop();
        }, 140);
      }, 2200 + Math.random() * 2800);
    };
    loop();
    return () => clearTimeout(t);
  }, [mood]);

  // Wander along the floor.
  useEffect(() => {
    const plan = WANDER[mood];
    const max = Math.max(0, stageWidth - spriteW);
    if (!scope.current || max === 0) return;
    x.current = Math.min(x.current, max);
    void animate(scope.current, { x: x.current }, { duration: 0 });
    if (!plan || reduce) return;

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const step = async () => {
      if (cancelled || !scope.current) return;
      const target = Math.random() * max;
      const dist = Math.abs(target - x.current);
      setFacing(target >= x.current ? 1 : -1);
      x.current = target;
      await animate(scope.current, { x: target }, { duration: dist / plan.speed, ease: "linear" });
      if (cancelled) return;
      timer = setTimeout(step, plan.every * (0.5 + Math.random()));
    };
    timer = setTimeout(step, 400 + Math.random() * 1200);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [mood, stageWidth, spriteW, reduce, animate, scope]);

  const onPet = () => {
    const id = ++heartSeq;
    setHearts((h) => [...h, id]);
    setTimeout(() => setHearts((h) => h.filter((n) => n !== id)), 1100);
  };

  const sleeping = mood === "sleepy";
  // Body motion per mood: bob while walking, hop when happy, shiver when worried.
  const body = reduce
    ? {}
    : mood === "happy"
      ? { y: [0, -14, 0], transition: { duration: 0.6, repeat: Infinity, repeatDelay: 0.5 } }
      : mood === "worried"
        ? { x: [0, -1, 1, 0], transition: { duration: 0.25, repeat: Infinity, repeatDelay: 1.2 } }
        : sleeping
          ? { scaleY: [1, 0.94, 1], transition: { duration: 2.4, repeat: Infinity, ease: "easeInOut" as const } }
          : { y: [0, -2, 0], transition: { duration: mood === "busy" || mood === "working" ? 0.3 : 0.7, repeat: Infinity } };

  return (
    <div ref={scope} className="absolute bottom-1 left-0" style={{ width: spriteW }}>
      <AnimatePresence>
        {say ? (
          <motion.div
            key={say.id}
            role="status"
            aria-live="polite"
            initial={{ opacity: 0, y: 6, scale: 0.9 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -4 }}
            transition={springBouncy}
            className="pointer-events-none absolute bottom-full left-1/2 z-10 mb-3 w-max max-w-56 -translate-x-1/2 rounded-xl bg-kumo-contrast px-2.5 py-1.5 text-xs font-medium text-kumo-inverse shadow-lg"
          >
            {say.text}
            <span className="absolute left-1/2 top-full -translate-x-1/2 border-x-[6px] border-t-[6px] border-x-transparent border-t-[var(--color-kumo-contrast)]" aria-hidden="true" />
          </motion.div>
        ) : null}
      </AnimatePresence>
      {/* effects */}
      <div className="pointer-events-none absolute -top-3 left-1/2 -translate-x-1/2">
        <AnimatePresence>
          {hearts.map((id) => (
            <motion.div
              key={id}
              className="absolute"
              initial={{ y: 0, opacity: 0, scale: 0.6 }}
              animate={{ y: -26, opacity: [0, 1, 0], scale: 1, x: (id % 3) * 6 - 6 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 1 }}
            >
              <PixelSprite sprite={HEART} scale={2} />
            </motion.div>
          ))}
        </AnimatePresence>
        {sleeping ? (
          <span className="absolute -top-2 left-3 font-mono text-xs font-bold text-kumo-subtle animate-float-up" aria-hidden="true">
            z
          </span>
        ) : null}
        {mood === "worried" || mood === "busy" ? (
          <motion.div
            className="absolute left-5 top-1"
            animate={reduce ? {} : { y: [0, 6], opacity: [1, 0] }}
            transition={{ duration: 0.9, repeat: Infinity, repeatDelay: 0.6 }}
          >
            <PixelSprite sprite={SWEAT} scale={2} />
          </motion.div>
        ) : null}
      </div>

      <button
        type="button"
        onClick={onPet}
        aria-label={`Pet ${def.name}`}
        title={`${def.name} is ${mood}`}
        className="relative block cursor-pointer rounded-md"
      >
        <motion.div
          style={{ scaleX: facing, originY: 1 }}
          animate={body}
          whileTap={{ scale: 0.88, transition: springBouncy }}
        >
          <PixelSprite sprite={blink || sleeping ? def.blink : def.open} scale={SCALE} />
        </motion.div>
        {mood === "working" ? (
          <motion.div
            className="absolute -top-3 left-1/2 -translate-x-1/2"
            animate={reduce ? {} : { y: [0, -2, 0] }}
            transition={{ duration: 0.3, repeat: Infinity }}
          >
            <PixelSprite sprite={BOX} scale={3} />
          </motion.div>
        ) : null}
      </button>
      {/* ground shadow */}
      <div className="mx-auto mt-0.5 h-1 w-3/4 rounded-full bg-kumo-fill opacity-70" aria-hidden="true" />
    </div>
  );
}

interface PetStageProps {
  mood: PetMood;
  pets?: readonly PetKind[];
  /** Latest thing to say; the speaker alternates between pets. */
  say?: Speech | null;
  className?: string;
}

/** A strip of floor the pets live on. Purely decorative apart from the "pet" buttons. */
export function PetStage({ mood, pets = ["ember", "miso"], say, className }: PetStageProps) {
  const [ref, width] = useWidth<HTMLDivElement>();
  return (
    <div
      ref={ref}
      className={cn("relative h-24 overflow-x-clip", className)}
      role="group"
      aria-label={`Pets: feeling ${mood}`}
    >
      <div className="absolute inset-x-0 bottom-1 h-px bg-kumo-hairline" aria-hidden="true" />
      {width > 0
        ? pets.map((kind, i) => (
            <Pet
              key={kind}
              kind={kind}
              mood={mood}
              stageWidth={width}
              start={(i + 1) / (pets.length + 1)}
              say={say && say.id % pets.length === i ? say : null}
            />
          ))
        : null}
    </div>
  );
}
