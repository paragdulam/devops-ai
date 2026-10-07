import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { closeTerminal, onTerminalData, openSshTerminal, resizeTerminal, writeTerminal } from "../lib/terminal";

// On-demand only — mounted the first time the Terminal tab is opened. Each open() call gets its own SSH PTY session on the
// Rust side (src-tauri/src/ssh/terminal.rs), closed when this unmounts.
export function TerminalPanel({ rentalId, onClose }: { rentalId: string; onClose: () => void }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    let cancelled = false;
    let sessionId: string | null = null;
    let unlisten: (() => void) | undefined;

    const term = new Terminal({ cursorBlink: true, convertEol: true });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerRef.current);
    fitAddon.fit();
    term.focus();

    const resizeObserver = new ResizeObserver(() => {
      // Hidden tabs report a zero size; fitting then would collapse the PTY.
      if (!containerRef.current?.offsetWidth) return;
      fitAddon.fit();
      if (sessionId) void resizeTerminal(sessionId, term.cols, term.rows);
    });
    resizeObserver.observe(containerRef.current);

    // Safety net: xterm only grabs keyboard focus on an explicit .focus()
    // call, and doesn't reclaim it if something else in the app steals it —
    // clicking back into the panel should always make it typeable again.
    const container = containerRef.current;
    const refocus = () => term.focus();
    container.addEventListener("mousedown", refocus);

    openSshTerminal(rentalId)
      .then(async (id) => {
        if (cancelled) {
          void closeTerminal(id);
          return;
        }
        sessionId = id;
        void resizeTerminal(id, term.cols, term.rows);
        unlisten = await onTerminalData(id, (chunk) => term.write(chunk));
        term.focus();
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });

    const dataDisposable = term.onData((data) => {
      if (sessionId) void writeTerminal(sessionId, data);
    });

    return () => {
      cancelled = true;
      resizeObserver.disconnect();
      container.removeEventListener("mousedown", refocus);
      dataDisposable.dispose();
      unlisten?.();
      if (sessionId) void closeTerminal(sessionId);
      term.dispose();
    };
  }, [rentalId]);

  return (
    <div className="remote-panel remote-panel--terminal">
      <div className="remote-panel__toolbar">
        <span>Terminal</span>
        <button type="button" onClick={onClose}>
          Close
        </button>
      </div>
      {error ? (
        <p className="validation-error">{error}</p>
      ) : (
        <div ref={containerRef} className="remote-panel__terminal-canvas" />
      )}
    </div>
  );
}
