import { useEffect, useState } from "react";
import { getChecklistSteps } from "../lib/rentalChecklist";
import { onProvisioningStep } from "../lib/provisioning";
import { RentalStatus } from "../types/rental";

const MARKERS: Record<string, string> = {
  done: "✓",
  current: "●",
  pending: "○",
  error: "✕",
};

export function RentalStateChecklist({ status, rentalId }: { status: RentalStatus; rentalId: string }) {
  const [currentPhase, setCurrentPhase] = useState<string | null>(null);

  useEffect(() => {
    if (status !== RentalStatus.PROVISIONING) return;
    let unlisten: (() => void) | undefined;
    onProvisioningStep(rentalId, (step) => {
      if (step.state === "start") setCurrentPhase(step.phase);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [rentalId, status]);

  const steps = getChecklistSteps(status, currentPhase);

  return (
    <ul className="rental-checklist">
      {steps.map((step) => (
        <li key={step.label} className={`rental-checklist__item is-${step.state}`}>
          <span className="rental-checklist__marker">{MARKERS[step.state]}</span>
          {step.label}
        </li>
      ))}
    </ul>
  );
}
