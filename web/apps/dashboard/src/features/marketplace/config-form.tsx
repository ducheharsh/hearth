import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Button } from "@cloudflare/kumo/components/button";
import { Switch } from "@cloudflare/kumo/components/switch";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { ArrowCounterClockwiseIcon, CaretDownIcon, EyeIcon, EyeSlashIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react";
import { Field, Select, TextArea, TextInput } from "@hearth/ui/components/form";
import type { AppConfig, AppConfigResponse, ServiceConfig } from "../../api/types";

interface ConfigFormProps {
  data: AppConfigResponse;
  value: AppConfig | null;
  /** `null` = recommended defaults. */
  onChange: (c: AppConfig | null) => void;
}

/** Docker settings for every service, pre-filled with the recommended values. */
export function ConfigForm({ data, value, onChange }: ConfigFormProps) {
  const services = value?.mode === "form" ? value.services : data.defaults;
  const raw = value?.mode === "raw" ? value.compose : data.compose;
  const mode = value?.mode ?? "form";

  const setService = (i: number, next: ServiceConfig) =>
    onChange({ mode: "form", services: services.map((s, j) => (j === i ? next : s)) });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <Tabs
          variant="segmented"
          size="sm"
          value={mode}
          onValueChange={(v) => onChange(v === "raw" ? { mode: "raw", compose: raw } : { mode: "form", services })}
          tabs={[
            { value: "form", label: "Form" },
            { value: "raw", label: "Compose YAML" },
          ]}
        />
        {value ? (
          <Button size="sm" variant="ghost" icon={<ArrowCounterClockwiseIcon />} onClick={() => onChange(null)}>
            Reset to Recommended
          </Button>
        ) : (
          <span className="text-xs text-kumo-subtle">Recommended settings</span>
        )}
      </div>

      {mode === "raw" ? (
        <Field label="docker-compose.yml" hint="Advanced: full control. Variables like ${APP_DATA} and ${VAULT} are filled in by hearth.">
          {(id) => (
            <TextArea
              id={id}
              name="compose"
              spellCheck={false}
              autoComplete="off"
              rows={16}
              value={raw}
              onChange={(e) => onChange({ mode: "raw", compose: e.target.value })}
            />
          )}
        </Field>
      ) : (
        services.map((s, i) => (
          <ServiceForm
            key={s.name}
            service={s}
            policies={data.restart_policies}
            onChange={(n) => setService(i, n)}
            only={services.length === 1}
          />
        ))
      )}
    </div>
  );
}

function ListEditor({
  label,
  items,
  placeholder,
  onChange,
  hint,
}: {
  label: string;
  items: string[];
  placeholder: string;
  onChange: (v: string[]) => void;
  hint: string;
}) {
  return (
    <fieldset className="flex flex-col gap-1.5">
      <legend className="mb-1.5 text-sm font-medium">{label}</legend>
      <AnimatePresence initial={false}>
        {items.map((item, i) => (
          <motion.div
            key={i}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="flex gap-1.5"
          >
            <TextInput
              aria-label={`${label} ${i + 1}`}
              value={item}
              spellCheck={false}
              autoComplete="off"
              className="font-mono text-xs"
              onChange={(e) => onChange(items.map((x, j) => (j === i ? e.target.value : x)))}
            />
            <Button
              size="sm"
              variant="ghost"
              shape="square"
              icon={<TrashIcon />}
              aria-label={`Remove ${label.toLowerCase()} ${i + 1}`}
              onClick={() => onChange(items.filter((_, j) => j !== i))}
            />
          </motion.div>
        ))}
      </AnimatePresence>
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs text-kumo-subtle">{hint}</p>
        <Button size="sm" variant="ghost" icon={<PlusIcon />} onClick={() => onChange([...items, ""])} aria-label={`Add ${label.toLowerCase()}`}>
          Add
        </Button>
      </div>
      {items.length === 0 ? <p className="sr-only">{placeholder}</p> : null}
    </fieldset>
  );
}

function ServiceForm({
  service: s,
  policies,
  onChange,
  only,
}: {
  service: ServiceConfig;
  policies: string[];
  onChange: (s: ServiceConfig) => void;
  only: boolean;
}) {
  const [reveal, setReveal] = useState(false);
  const [open, setOpen] = useState(only);
  const set = <K extends keyof ServiceConfig>(k: K, v: ServiceConfig[K]) => onChange({ ...s, [k]: v });

  return (
    <div className={open ? "rounded-xl bg-kumo-elevated ring-1 ring-kumo-hairline" : "rounded-xl ring-1 ring-kumo-hairline"}>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between gap-3 rounded-xl px-4 py-3 text-left hover:bg-kumo-tint"
      >
        <span className="min-w-0">
          <span className="font-medium">{s.name}</span>
          <span className="ml-2 truncate font-mono text-xs text-kumo-subtle" translate="no">
            {s.image}:{s.tag}
          </span>
        </span>
        <motion.span animate={{ rotate: open ? 180 : 0 }} className="inline-flex text-kumo-subtle">
          <CaretDownIcon size={16} aria-hidden="true" />
        </motion.span>
      </button>
      <AnimatePresence initial={false}>
      {open ? (
      <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden">
      <div className="grid grid-cols-1 gap-4 px-4 pb-4 sm:grid-cols-2">
        <Field label="Image" hint="Repository, without the tag.">
          {(id) => <TextInput id={id} value={s.image} spellCheck={false} autoComplete="off" className="font-mono text-xs" onChange={(e) => set("image", e.target.value)} />}
        </Field>
        <Field label="Version (tag)" hint="Pin a version for predictable updates.">
          {(id) => <TextInput id={id} value={s.tag} spellCheck={false} autoComplete="off" className="font-mono text-xs" onChange={(e) => set("tag", e.target.value)} />}
        </Field>
        <Field label="Restart policy">
          {() => (
            <Select aria-label="Restart policy" value={s.restart} items={Object.fromEntries(policies.map((p) => [p, p]))} onChange={(v) => set("restart", v)} />
          )}
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Memory limit" hint="e.g. 512m">
            {(id) => <TextInput id={id} value={s.mem_limit ?? ""} placeholder="none…" autoComplete="off" onChange={(e) => set("mem_limit", e.target.value || null)} />}
          </Field>
          <Field label="CPU limit" hint="e.g. 0.5">
            {(id) => <TextInput id={id} value={s.cpus ?? ""} placeholder="none…" inputMode="decimal" autoComplete="off" onChange={(e) => set("cpus", e.target.value || null)} />}
          </Field>
        </div>

        <fieldset className="flex flex-col gap-1.5 sm:col-span-2">
          <legend className="mb-1.5 flex w-full items-center justify-between text-sm font-medium">
            Environment variables
            {s.environment.some((e) => e.secret) ? (
              <Button
                size="xs"
                variant="ghost"
                icon={reveal ? <EyeSlashIcon /> : <EyeIcon />}
                onClick={() => setReveal((r) => !r)}
                aria-pressed={reveal}
              >
                {reveal ? "Hide Secrets" : "Show Secrets"}
              </Button>
            ) : null}
          </legend>
          {s.environment.map((e, i) => (
            <div key={i} className="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)_auto] gap-1.5">
              <TextInput
                aria-label={`Variable ${i + 1} name`}
                value={e.key}
                spellCheck={false}
                autoComplete="off"
                className="font-mono text-xs"
                onChange={(ev) => set("environment", s.environment.map((x, j) => (j === i ? { ...x, key: ev.target.value } : x)))}
              />
              <TextInput
                aria-label={`${e.key || `Variable ${i + 1}`} value`}
                value={e.value}
                type={e.secret && !reveal ? "password" : "text"}
                spellCheck={false}
                autoComplete="off"
                className="font-mono text-xs"
                onChange={(ev) => set("environment", s.environment.map((x, j) => (j === i ? { ...x, value: ev.target.value } : x)))}
              />
              <Button
                size="sm"
                variant="ghost"
                shape="square"
                icon={<TrashIcon />}
                aria-label={`Remove ${e.key || "variable"}`}
                onClick={() => set("environment", s.environment.filter((_, j) => j !== i))}
              />
            </div>
          ))}
          <div className="flex items-center justify-between">
            <p className="text-xs text-kumo-subtle">
              <code>{"${APP_SECRET}"}</code> is a strong random value hearth keeps for this app.
            </p>
            <Button size="sm" variant="ghost" icon={<PlusIcon />} onClick={() => set("environment", [...s.environment, { key: "", value: "", secret: false }])}>
              Add Variable
            </Button>
          </div>
        </fieldset>

        <ListEditor label="Ports" items={s.ports} placeholder="host:container" hint="Usually not needed: the gateway reaches apps privately." onChange={(v) => set("ports", v)} />
        <ListEditor label="Volumes" items={s.volumes} placeholder="${APP_DATA}:/data" hint="host path : container path" onChange={(v) => set("volumes", v)} />

        <Field label="Command override" hint="Leave empty to use the image's default." className="sm:col-span-2">
          {(id) => <TextInput id={id} value={s.command ?? ""} spellCheck={false} autoComplete="off" className="font-mono text-xs" placeholder="default…" onChange={(e) => set("command", e.target.value || null)} />}
        </Field>
        <div className="flex flex-wrap gap-6 sm:col-span-2">
          <Switch label="Read-only filesystem" checked={s.read_only} onCheckedChange={(v) => set("read_only", v)} />
          <Switch label="Privileged (full host access)" checked={s.privileged} onCheckedChange={(v) => set("privileged", v)} />
        </div>
      </div>
      </motion.div>
      ) : null}
      </AnimatePresence>
    </div>
  );
}
