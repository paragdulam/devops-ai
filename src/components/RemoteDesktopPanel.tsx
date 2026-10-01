import { useEffect, useRef, useState } from "react";
import RFB from "@novnc/novnc";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { useRentalState } from "../state/RentalContext";
import { RentalStatus } from "../types/rental";
import { StopRentingButton } from "./StopRentingButton";

// X11 keysyms used to synthesize the VM-side paste chord.
const XK_SHIFT_L = 0xffe1;
const XK_INSERT = 0xff63;
const XK_ALT_L = 0xffe9;
const XK_SUPER_L = 0xffeb;

const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform);

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
    // Fit the remote screen to the panel; noVNC then translates pointer
    // positions back to framebuffer coordinates.
    rfb.scaleViewport = true;

    // Host -> VM clipboard: push the local clipboard to the VM whenever the
    // user comes back to the app or moves onto the desktop, so a plain paste
    // inside the VM gets whatever was last copied on this computer. Non-text
    // clipboard contents (e.g. images) make `readText` reject — ignored.
    let lastSynced: string | null = null;
    const syncClipboard = async (force = false) => {
      let text: string;
      try {
        text = await readText();
      } catch {
        return;
      }
      if (!text || (!force && text === lastSynced)) return;
      lastSynced = text;
      rfb.clipboardPasteFrom(text);
    };
    const handleSyncTrigger = () => void syncClipboard();

    // On macOS noVNC forwards ⌘ as Alt, so ⌘V would reach the VM as Alt+V and
    // paste nothing. Intercept it (capture phase, ahead of noVNC's own canvas
    // listener), sync the clipboard, release the held ⌘, and send Shift+Insert
    // — the X11 paste chord that works in GTK apps *and* terminals, where
    // Ctrl+V doesn't paste.
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!IS_MAC || !e.metaKey || e.ctrlKey || e.altKey || e.code !== "KeyV") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      void syncClipboard(true).then(() => {
        rfb.sendKey(XK_ALT_L, "MetaLeft", false);
        rfb.sendKey(XK_SUPER_L, "MetaRight", false);
        rfb.sendKey(XK_SHIFT_L, "ShiftLeft", true);
        rfb.sendKey(XK_INSERT, "Insert");
        rfb.sendKey(XK_SHIFT_L, "ShiftLeft", false);
      });
    };

    const screen = screenRef.current;
    screen.addEventListener("keydown", handleKeyDown, true);
    screen.addEventListener("pointerenter", handleSyncTrigger);
    window.addEventListener("focus", handleSyncTrigger);

    const handleConnect = () => {
      setConnected(true);
      setStatusMessage("");
      void syncClipboard();
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
      screen.removeEventListener("keydown", handleKeyDown, true);
      screen.removeEventListener("pointerenter", handleSyncTrigger);
      window.removeEventListener("focus", handleSyncTrigger);
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
