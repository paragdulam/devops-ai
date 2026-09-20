import { useEffect, useState } from "react";
import { useRentalState } from "../state/RentalContext";
import { RentalStateChecklist } from "../components/RentalStateChecklist";
import { RemoteDesktopPanel } from "../components/RemoteDesktopPanel";
import { TerminalPanel } from "../components/TerminalPanel";
import { ProvisioningLogPanel } from "../components/ProvisioningLogPanel";
import { RentalTimer } from "../components/RentalTimer";
import { StopRentingButton } from "../components/StopRentingButton";
import { RentalStatus } from "../types/rental";

export function RentalScreen() {
  const { state, dispatch, rentalService } = useRentalState();
  const rental = state.rental;
  const [desktopExpanded, setDesktopExpanded] = useState(false);
  const [logsOpen, setLogsOpen] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);

  useEffect(() => {
    if (!rental) return;
    const unsubscribe = rentalService.subscribeToRental(rental.id, (updated) => {
      dispatch({ type: "RENTAL_UPDATED", rental: updated });
      if (updated.status === RentalStatus.RELEASED) {
        dispatch({ type: "RESET" });
      }
    });
    return unsubscribe;
    // Only re-subscribe when the rental id itself changes, not on every update.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rental?.id]);

  if (!rental) return null;

  const isRunning = rental.status === RentalStatus.RUNNING;

  return (
    <main className={`rental-screen${desktopExpanded ? " rental-screen--desktop-expanded" : ""}`}>
      {!desktopExpanded && (
        <header className="rental-screen__header">
          <h1>{rental.projectName}</h1>
          {isRunning && <span className="rental-badge">● RENTING</span>}
          <p>AWS EC2 · {rental.machineProfile}</p>
          {rental.startedAt && <RentalTimer since={rental.startedAt} />}
        </header>
      )}

      {!desktopExpanded && (
        <>
          <RentalStateChecklist status={rental.status} rentalId={rental.id} />
          {rental.status === RentalStatus.FAILED && rental.error && (
            <p className="validation-error">{rental.error}</p>
          )}
          <div className="rental-screen__on-demand-actions">
            <button type="button" onClick={() => setLogsOpen((v) => !v)}>
              {logsOpen ? "Hide Logs" : "View Logs"}
            </button>
            <button type="button" onClick={() => setTerminalOpen((v) => !v)}>
              {terminalOpen ? "Close Terminal" : "Open Terminal"}
            </button>
          </div>
        </>
      )}

      <section className="rental-screen__panels">
        <RemoteDesktopPanel
          rentalId={rental.id}
          expanded={desktopExpanded}
          onToggleExpand={() => setDesktopExpanded((v) => !v)}
        />
        {!desktopExpanded && logsOpen && (
          <ProvisioningLogPanel rentalId={rental.id} onClose={() => setLogsOpen(false)} />
        )}
        {!desktopExpanded && terminalOpen && (
          <TerminalPanel rentalId={rental.id} onClose={() => setTerminalOpen(false)} />
        )}
      </section>

      {!desktopExpanded && (
        <footer className="rental-screen__footer">
          <StopRentingButton />
        </footer>
      )}
    </main>
  );
}
