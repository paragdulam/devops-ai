import { useEffect, useRef, useState } from "react";
import { getProvisioningLog, onProvisioningLog } from "../lib/provisioning";

// On-demand only — mounted while `open`, unmounted (and unsubscribed) when
// closed. Replays history-so-far via get_provisioning_log, then layers live
// events on top, so opening late still shows everything emitted before.
export function ProvisioningLogPanel({ rentalId, onClose }: { rentalId: string; onClose: () => void }) {
  const [lines, setLines] = useState<string[]>([]);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    getProvisioningLog(rentalId).then((replay) => {
      if (!cancelled) setLines(replay);
    });

    onProvisioningLog(rentalId, (line) => {
      if (!cancelled) setLines((prev) => [...prev, line]);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [rentalId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  return (
    <div className="provisioning-log-panel">
      <div className="provisioning-log-panel__toolbar">
        <span>Provisioning logs</span>
        <button type="button" onClick={onClose}>
          Close
        </button>
      </div>
      <pre className="provisioning-log-panel__body">
        {lines.join("\n")}
        <div ref={bottomRef} />
      </pre>
    </div>
  );
}
