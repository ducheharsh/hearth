import { useEffect, useRef, useState } from "react";
import { Button } from "@cloudflare/kumo/components/button";
import { ArrowClockwiseIcon } from "@phosphor-icons/react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useTheme } from "@hearth/ui/theme";

const DARK = { background: "#0b0b0c", foreground: "#e7e5e4", cursor: "#fb923c", selectionBackground: "#44403c" };
const LIGHT = { background: "#fafaf9", foreground: "#1c1917", cursor: "#c2410c", selectionBackground: "#d6d3d1" };

type Status = "connecting" | "open" | "closed";

/** Interactive shell in a container (docker exec over WebSocket). Lazy-loaded: xterm is heavy. */
export default function ContainerTerminal({ id }: { id: string }) {
  const host = useRef<HTMLDivElement>(null);
  const { resolved } = useTheme();
  const [status, setStatus] = useState<Status>("connecting");
  const [session, setSession] = useState(0);
  const termRef = useRef<XTerm | null>(null);

  useEffect(() => {
    if (!host.current) return;
    const term = new XTerm({
      cursorBlink: true,
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 1.25,
      scrollback: 5000,
      theme: resolved === "dark" ? DARK : LIGHT,
    });
    termRef.current = term;
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host.current);
    fit.fit();
    term.focus();

    const proto = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${proto}://${location.host}/api/containers/${encodeURIComponent(id)}/terminal`);
    ws.binaryType = "arraybuffer";
    const enc = new TextEncoder();
    const sendSize = () => {
      if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }));
    };
    setStatus("connecting");
    ws.onopen = () => {
      setStatus("open");
      sendSize();
    };
    ws.onmessage = (ev) => term.write(typeof ev.data === "string" ? ev.data : new Uint8Array(ev.data as ArrayBuffer));
    ws.onclose = () => {
      setStatus("closed");
      term.write("\r\n\x1b[2m[session ended]\x1b[0m\r\n");
    };
    const input = term.onData((d) => ws.readyState === WebSocket.OPEN && ws.send(enc.encode(d)));
    const resize = term.onResize(sendSize);
    const ro = new ResizeObserver(() => fit.fit());
    ro.observe(host.current);

    return () => {
      ro.disconnect();
      input.dispose();
      resize.dispose();
      ws.close();
      term.dispose();
      termRef.current = null;
    };
    // `resolved` is applied live below; a theme change shouldn't restart the shell.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, session]);

  useEffect(() => {
    if (termRef.current) termRef.current.options.theme = resolved === "dark" ? DARK : LIGHT;
  }, [resolved]);

  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex items-center justify-between gap-2 text-xs text-kumo-subtle">
        <span aria-live="polite">
          {status === "open" ? "Connected: you’re inside the container as its default user." : status === "connecting" ? "Connecting…" : "Disconnected."}
        </span>
        {status === "closed" ? (
          <Button size="xs" icon={<ArrowClockwiseIcon />} onClick={() => setSession((s) => s + 1)}>
            Reconnect
          </Button>
        ) : null}
      </div>
      <div ref={host} className="min-h-0 flex-1 overflow-hidden rounded-lg bg-[#0b0b0c] p-2 ring-1 ring-kumo-hairline" aria-label="Container terminal" />
    </div>
  );
}
