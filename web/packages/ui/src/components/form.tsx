import { useId, useMemo, useState, type ComponentProps, type ReactNode } from "react";
import { Combobox } from "@cloudflare/kumo/components/combobox";
import { Select as KumoSelect } from "@cloudflare/kumo/components/select";
import { cn } from "@cloudflare/kumo/utils";

interface FieldProps {
  label: string;
  hint?: ReactNode;
  error?: string | null;
  children: (id: string) => ReactNode;
  className?: string;
}

/** Label + control + hint/error, with ids wired for accessibility. */
export function Field({ label, hint, error, children, className }: FieldProps) {
  const id = useId();
  return (
    <div className={cn("flex min-w-0 flex-col gap-1.5", className)}>
      <label htmlFor={id} className="text-sm font-medium text-kumo-default">
        {label}
      </label>
      {children(id)}
      {error ? (
        <p className="text-xs text-kumo-danger" role="alert">
          {error}
        </p>
      ) : hint ? (
        <p className="text-xs text-kumo-subtle text-pretty">{hint}</p>
      ) : null}
    </div>
  );
}

const control =
  "h-8 w-full min-w-0 rounded-lg bg-kumo-base px-3 text-sm text-kumo-default ring-1 ring-kumo-line transition-[box-shadow] placeholder:text-kumo-placeholder focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kumo-brand disabled:opacity-60";

/** Plain text input styled like Kumo (for dense forms and tables). */
export function TextInput({ className, ...props }: ComponentProps<"input">) {
  return <input {...props} className={cn(control, className)} />;
}

export function TextArea({ className, ...props }: ComponentProps<"textarea">) {
  return (
    <textarea
      {...props}
      className={cn(control, "h-auto min-h-40 py-2 font-mono text-xs leading-relaxed", className)}
    />
  );
}

interface SelectProps {
  value: string;
  onChange: (v: string) => void;
  /** value → label */
  items: Record<string, ReactNode>;
  "aria-label": string;
  className?: string;
  size?: "xs" | "sm" | "base";
  placeholder?: string;
  disabled?: boolean;
}

/** Short option lists (≤ ~15). Kumo Select (Base UI): keyboard, typeahead, themed popover. */
export function Select({ value, onChange, items, className, size = "base", placeholder, disabled, ...rest }: SelectProps) {
  return (
    <KumoSelect
      aria-label={rest["aria-label"]}
      className={cn("w-full", className)}
      size={size}
      value={value}
      placeholder={placeholder}
      disabled={disabled}
      onValueChange={(v: unknown) => onChange(typeof v === "string" ? v : "")}
      items={items}
    />
  );
}

interface SearchSelectProps {
  value: string;
  onChange: (v: string) => void;
  /** Option values; `label` maps a value to its display text. */
  options: readonly string[];
  label?: (v: string) => string;
  "aria-label": string;
  placeholder?: string;
  className?: string;
}

/** Long option lists (time zones, categories): type to filter. Kumo Combobox. */
export function SearchSelect({ value, onChange, options, label = (v) => v, placeholder, className, ...rest }: SearchSelectProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? options.filter((o) => label(o).toLowerCase().includes(q)) : options;
  }, [options, query, label]);
  return (
    <div className={cn("w-full", className)}>
      <Combobox
        value={value}
        onValueChange={(v: unknown) => onChange(typeof v === "string" ? v : "")}
        items={filtered as string[]}
        itemToStringLabel={(v: unknown) => label(String(v))}
        onInputValueChange={(v: string) => setQuery(v)}
      >
        <Combobox.TriggerInput aria-label={rest["aria-label"]} placeholder={placeholder ?? "Search…"} />
        <Combobox.Content>
          <Combobox.Empty />
          <Combobox.List>
            {(item: string) => (
              <Combobox.Item key={item} value={item}>
                {label(item)}
              </Combobox.Item>
            )}
          </Combobox.List>
        </Combobox.Content>
      </Combobox>
    </div>
  );
}
