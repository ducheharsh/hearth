import { useState, type FormEvent } from "react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Switch } from "@cloudflare/kumo/components/switch";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { PlusIcon, TrashIcon, WarningIcon, XIcon } from "@phosphor-icons/react";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { Field, TextArea, TextInput } from "@hearth/ui/components/form";
import { useCreateCustom } from "../../api/hooks";

const COMPOSE_EXAMPLE = `services:
  web:
    image: ghcr.io/you/your-app:latest
    environment:
      TZ: \${TZ}
    volumes:
      - \${APP_DATA}:/data
`;

/** Bring your own app: a Docker image, or a full compose file. It then installs like any other. */
export function CustomAppDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const create = useCreateCustom();
  const [mode, setMode] = useState<"image" | "compose">("image");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [image, setImage] = useState("");
  const [compose, setCompose] = useState(COMPOSE_EXAMPLE);
  const [service, setService] = useState("web");
  const [containerPort, setContainerPort] = useState("80");
  const [port, setPort] = useState("4100");
  const [ram, setRam] = useState("256");
  const [env, setEnv] = useState<[string, string][]>([["TZ", "${TZ}"]]);
  const [protect, setProtect] = useState(true);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    create.mutate(
      {
        name,
        description,
        port: Number(port),
        container_port: Number(containerPort),
        ram_mb: Number(ram) || 0,
        protect,
        ...(mode === "image"
          ? { image, env: env.filter(([k]) => k.trim()), volumes: ["${APP_DATA}:/data"] }
          : { compose, service }),
      },
      { onSuccess: () => onOpenChange(false) },
    );
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog size="xl" className={DIALOG_FRAME}>
        <DialogFrame>
        <form onSubmit={submit} className="flex flex-col gap-5">
          <div className="flex items-start justify-between gap-3">
            <div>
              <Dialog.Title className="text-lg font-semibold">Add Your Own App</Dialog.Title>
              <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
                Any Docker image or compose file. It gets the same capacity check, login and address settings as catalog apps.
              </Dialog.Description>
            </div>
            <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label="Name">
              {(id) => <TextInput id={id} name="name" required value={name} autoComplete="off" placeholder="My App…" onChange={(e) => setName(e.target.value)} />}
            </Field>
            <Field label="Description">
              {(id) => <TextInput id={id} name="description" value={description} autoComplete="off" placeholder="What it does…" onChange={(e) => setDescription(e.target.value)} />}
            </Field>
          </div>

          <Tabs
            variant="segmented"
            size="sm"
            value={mode}
            onValueChange={(v) => setMode(v as "image" | "compose")}
            tabs={[
              { value: "image", label: "Docker Image" },
              { value: "compose", label: "Compose File" },
            ]}
          />

          {mode === "image" ? (
            <div className="flex flex-col gap-4">
              <Field label="Image" hint="e.g. nginx:1.27 or ghcr.io/owner/app:v2">
                {(id) => <TextInput id={id} name="image" required value={image} spellCheck={false} autoComplete="off" className="font-mono text-xs" placeholder="ghcr.io/owner/app:latest…" onChange={(e) => setImage(e.target.value)} />}
              </Field>
              <fieldset className="flex flex-col gap-1.5">
                <legend className="mb-1.5 text-sm font-medium">Environment</legend>
                {env.map(([k, v], i) => (
                  <div key={i} className="grid grid-cols-[1fr_1.5fr_auto] gap-1.5">
                    <TextInput aria-label={`Variable ${i + 1} name`} value={k} spellCheck={false} autoComplete="off" className="font-mono text-xs" onChange={(e) => setEnv(env.map((x, j) => (j === i ? [e.target.value, x[1]] : x)))} />
                    <TextInput aria-label={`Variable ${i + 1} value`} value={v} spellCheck={false} autoComplete="off" className="font-mono text-xs" onChange={(e) => setEnv(env.map((x, j) => (j === i ? [x[0], e.target.value] : x)))} />
                    <Button size="sm" variant="ghost" shape="square" icon={<TrashIcon />} aria-label="Remove variable" onClick={() => setEnv(env.filter((_, j) => j !== i))} />
                  </div>
                ))}
                <Button size="sm" variant="ghost" icon={<PlusIcon />} className="self-end" onClick={() => setEnv([...env, ["", ""]])}>
                  Add Variable
                </Button>
              </fieldset>
              <p className="text-xs text-kumo-subtle">
                Data is kept in <code>{"${APP_DATA}"}</code> mounted at <code>/data</code>. Use the Compose tab for anything more complex.
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-[2fr_1fr]">
              <Field label="docker-compose.yml" hint="Variables: ${APP_DATA}, ${APP_SECRET}, ${VAULT}, ${MEDIA}, ${BACKUPS}, ${TZ}">
                {(id) => <TextArea id={id} name="compose" rows={12} spellCheck={false} value={compose} onChange={(e) => setCompose(e.target.value)} />}
              </Field>
              <Field label="Service to open" hint="Which service the gateway forwards to.">
                {(id) => <TextInput id={id} required value={service} spellCheck={false} autoComplete="off" onChange={(e) => setService(e.target.value)} />}
              </Field>
            </div>
          )}

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <Field label="Container port" hint="The port the app listens on inside.">
              {(id) => <TextInput id={id} inputMode="numeric" required value={containerPort} onChange={(e) => setContainerPort(e.target.value.replace(/\D/g, ""))} />}
            </Field>
            <Field label="Tailnet port" hint="https://…ts.net:port">
              {(id) => <TextInput id={id} inputMode="numeric" required value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} />}
            </Field>
            <Field label="Expected RAM (MB)" hint="Used by the capacity check.">
              {(id) => <TextInput id={id} inputMode="numeric" value={ram} onChange={(e) => setRam(e.target.value.replace(/\D/g, ""))} />}
            </Field>
          </div>
          <Switch label="Require my login (recommended)" checked={protect} onCheckedChange={setProtect} />

          {create.error ? <Banner variant="error" icon={<WarningIcon />} title="Couldn’t add the app" description={create.error.message} /> : null}

          <div className="flex justify-end gap-2">
            <Dialog.Close render={(p) => <Button {...p} variant="secondary">Cancel</Button>} />
            <Button type="submit" variant="primary" icon={<PlusIcon />} loading={create.isPending}>
              Add App
            </Button>
          </div>
        </form>
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}
