# @hearth/ui: design system

Shared UI for every hearth frontend. **Kumo** (Cloudflare's React components, on Base UI +
Tailwind v4) provides the primitives; **Geist** (Vercel) provides the type; this package
adds hearth's tokens, motion language, data-viz bits and the pets.

## Foundations

| Layer | Source | Rule |
|---|---|---|
| Color | Kumo semantic tokens (`bg-kumo-base`, `text-kumo-subtle`, `ring-kumo-hairline`…) | Never raw palette classes; never `dark:` (Kumo uses `light-dark()` + `[data-mode]`) |
| Type | Geist / Geist Mono variable fonts | Numbers in data use `tabular-nums` |
| Radius | `rounded-xl` surfaces, `rounded-lg` icons/code, `rounded-full` pills/dots | |
| Motion | `motion.ts` (`springSnappy`, `springSoft`, `springBouncy`, `riseIn`, `pressScale`, `hoverLift`) | Animate only transform/opacity; honor `prefers-reduced-motion` |
| Theme | `ThemeProvider` / `ThemeToggle` (system → light → dark), inline pre-paint script in `index.html` | |
| Format | `format.ts` (`Intl` only) | No hand-rolled number/date formats |

## Components (`@hearth/ui/components/<name>`)

| Component | Use |
|---|---|
| `Card` / `InteractiveCard` | Static panel / actionable tile with hover lift |
| `StatTile` | Label + icon + big figure + hint + viz slot |
| `AnimatedNumber` | Spring-animated figures (pass a module-level `format`) |
| `Sparkline` | Tiny area chart, reveals on mount and morphs on updates |
| `UsageBar` (+ `toneFor`) | Accessible meter (`role="meter"`), optional projected overlay |
| `StatusDot` | Health dot with optional live pulse |
| `Pressable` | Shared press feedback around any control |
| `CategoryIcon` | Category tile (icon + Kumo badge tint) |
| `Section` | Page section heading row |

Import per file (`@hearth/ui/components/card`) rather than from a barrel, and use Kumo's
granular paths (`@cloudflare/kumo/components/button`) for the same reason.

## Pets (`@hearth/ui/pets`)

`<PetStage mood=… />` renders **Ember** (a little hearth flame) and **Miso** (a cat):
original pixel-art sprites drawn as SVG (`sprites.ts`, merged into row runs).
Moods: `sleepy` (idle machine, zzz), `idle` (wander, blink), `busy` (fast + sweat),
`worried` (memory/disk pressure: shiver + sweat), `working` (carrying a box during
installs), `happy` (hops after a success). Click/tap a pet for hearts. Fully static
under reduced motion; can be hidden from the header.

Adding a sprite: draw rows of palette keys (`.` = transparent), all rows the same width;
make a `blink` frame by swapping the eye pixels.
