import { useEffect, useRef, useState } from "react";
import RFB from "@novnc/novnc";
import { useRentalState } from "../state/RentalContext";
import { RentalStatus } from "../types/rental";
import { StopRentingButton } from "./StopRentingButton";

// RFB owns `.remote-panel__screen-canvas` exclusively (it replaces the node's
// contents with its own canvas) — status text renders as a separate overlay
// sibling rather than mixing React-rendered children into that node.
export function RemoteDesktopPanel({
  rentalId,
  expanded,
  onToggleExpand,
}: {
  rentalId: string;
  expanded: boolean;
  onToggleExpand: () => void;
}) {
  const { state } = useRentalState();
  const screenRef = useRef<HTMLDivElement>(null);
  const [connected, setConnected] = useState(false);
  const [statusMessage, setStatusMessage] = useState(
    "Waiting for the remote machine to become ready…",
  );

  const rental = state.rental;
  const connection = rental?.connection ?? null;
  const canConnect =
    connection !== null &&
    (rental?.status === RentalStatus.READY || rental?.status === RentalStatus.RUNNING);

  useEffect(() => {
    if (!canConnect || !connection || !screenRef.current) return;

    const url = `ws://${connection.publicIp}:${connection.vncPort}/`;
    const rfb = new RFB(screenRef.current, url, {
      credentials: { password: connection.vncPassword },
    });

    const handleConnect = () => {
      setConnected(true);
      setStatusMessage("");
    };
    const handleDisconnect = (e: Event) => {
      const detail = (e as CustomEvent<{ clean: boolean }>).detail;
      setConnected(false);
      setStatusMessage(
        detail.clean
          ? "Waiting for the remote machine to become ready…"
          : "Lost connection to the remote desktop.",
      );
    };
    const handleSecurityFailure = () => {
      setConnected(false);
      setStatusMessage("Could not authenticate to the remote desktop.");
    };
    const handleCredentialsRequired = () => {
      rfb.sendCredentials({ password: connection.vncPassword });
    };

    rfb.addEventListener("connect", handleConnect);
    rfb.addEventListener("disconnect", handleDisconnect);
    rfb.addEventListener("securityfailure", handleSecurityFailure);
    rfb.addEventListener("credentialsrequired", handleCredentialsRequired);

    return () => {
      rfb.removeEventListener("connect", handleConnect);
      rfb.removeEventListener("disconnect", handleDisconnect);
      rfb.removeEventListener("securityfailure", handleSecurityFailure);
      rfb.removeEventListener("credentialsrequired", handleCredentialsRequired);
      rfb.disconnect();
    };
    // Reconnect only when the connection target itself changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connection?.publicIp, connection?.vncPort, connection?.vncPassword, canConnect]);

  return (
    <div className={`remote-panel remote-panel--desktop${expanded ? " remote-panel--expanded" : ""}`}>
      <div className="remote-panel__toolbar">
        <button type="button" onClick={onToggleExpand}>
          {expanded ? "Shrink" : "Full Screen"}
        </button>
        {expanded && <StopRentingButton label="Disconnect" />}
      </div>
      <div className="remote-panel__screen">
        <div ref={screenRef} className="remote-panel__screen-canvas" data-rental-id={rentalId} />
        {!connected && <div className="remote-panel__screen-overlay">{statusMessage}</div>}
      </div>
    </div>
  );
}
