import { useEffect, useRef, useState } from "react";
import { useRentalState } from "../state/RentalContext";
import { isLive } from "../state/rentalReducer";
import { RentalStateChecklist } from "../components/RentalStateChecklist";
import { RemoteDesktopPanel } from "../components/RemoteDesktopPanel";
import { TerminalPanel } from "../components/TerminalPanel";
import { ProvisioningLogPanel } from "../components/ProvisioningLogPanel";
import { RentalTimer } from "../components/RentalTimer";
import { RentalActualCost, RentalCostEstimate } from "../components/RentalCost";
import { StopRentingButton } from "../components/StopRentingButton";
import { RentalStatus, type Rental } from "../types/rental";

type Tab = "progress" | "desktop" | "terminal";

const TABS: { id: Tab; label: string }[] = [
  { id: "progress", label: "Progress" },
  { id: "desktop", label: "Desktop" },
  { id: "terminal", label: "Terminal" },
];

// One instance per rental, all kept mounted by WorkspaceScreen (the unselected
// ones just hidden) so a VM's VNC and SSH sessions stay connected while the
// user works in another.
export function VmDetail({ rental, visible }: { rental: Rental; visible: boolean }) {
  const { dispatch } = useRentalState();
  const live = isLive(rental);
  const [tab, setTab] = useState<Tab>(live ? "desktop" : "progress");
  const [logsOpen, setLogsOpen] = useState(false);
  const [desktopExpanded, setDesktopExpanded] = useState(false);
  // The terminal opens an SSH session, so it is only mounted once asked for.
  const [terminalOpened, setTerminalOpened] = useState(false);
  const wasLive = useRef(live);

  useEffect(() => {
    if (live && !wasLive.current) setTab("desktop");
    wasLive.current = live;
  }, [live]);

  function selectTab(next: Tab) {
    if (next === "terminal") setTerminalOpened(true);
    setTab(next);
  }

  function closeTerminal() {
    setTerminalOpened(false);
    setTab("progress");
  }

  return (
    <section className="vm-detail" hidden={!visible}>
      <header className="vm-detail__header">
        <div>
          <h1>{rental.githubRepo?.fullName ?? rental.projectName}</h1>
          <p>
            AWS EC2 · {rental.machineProfile}
            {rental.status === RentalStatus.RUNNING && (
              <span className="rental-badge"> ● RENTING</span>
            )}
            {rental.startedAt && (
              <>
                {" · "}
                <RentalTimer since={rental.startedAt} />
              </>
            )}
          </p>
          <p className="vm-detail__cost">
            <RentalCostEstimate rental={rental} />
            {rental.launchedAt && <RentalActualCost rental={rental} />}
          </p>
        </div>
        {rental.status !== RentalStatus.FAILED && <StopRentingButton rentalId={rental.id} />}
      </header>

      <div className="vm-tabs" role="tablist">
        {TABS.map(({ id, label }) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className={tab === id ? "vm-tabs__tab active" : "vm-tabs__tab"}
            disabled={id !== "progress" && !live}
            onClick={() => selectTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="vm-detail__pane vm-detail__pane--scroll" hidden={tab !== "progress"}>
        <RentalStateChecklist status={rental.status} rentalId={rental.id} />
        {rental.status === RentalStatus.FAILED && (
          <>
            {rental.error && <p className="validation-error">{rental.error}</p>}
            <button
              type="button"
              onClick={() => dispatch({ type: "RENTAL_REMOVED", rentalId: rental.id })}
            >
              Dismiss
            </button>
          </>
        )}
        <div className="vm-detail__actions">
          <button type="button" onClick={() => setLogsOpen((v) => !v)}>
            {logsOpen ? "Hide Logs" : "View Logs"}
          </button>
        </div>
        {logsOpen && <ProvisioningLogPanel rentalId={rental.id} onClose={() => setLogsOpen(false)} />}
      </div>

      <div className="vm-detail__pane" hidden={tab !== "desktop"}>
        <RemoteDesktopPanel
          rental={rental}
          expanded={desktopExpanded}
          onToggleExpand={() => setDesktopExpanded((v) => !v)}
        />
      </div>

      <div className="vm-detail__pane" hidden={tab !== "terminal"}>
        {terminalOpened && <TerminalPanel rentalId={rental.id} onClose={closeTerminal} />}
      </div>
    </section>
  );
}
