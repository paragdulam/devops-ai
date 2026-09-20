import { useState } from "react";
import { useRentalState } from "../state/RentalContext";
import { ConfirmDialog } from "./ConfirmDialog";

export function StopRentingButton({ label = "STOP RENTING" }: { label?: string }) {
  const { state, rentalService } = useRentalState();
  const [confirming, setConfirming] = useState(false);
  const [stopping, setStopping] = useState(false);

  if (!state.rental) return null;

  async function handleConfirm() {
    setConfirming(false);
    setStopping(true);
    await rentalService.stopRental(state.rental!.id);
    // The RENTAL_UPDATED subscription (in RentalScreen) carries the
    // STOPPING -> RELEASED transition and resets state back to Home.
  }

  return (
    <>
      <button
        type="button"
        className="stop-renting-button"
        disabled={stopping}
        onClick={() => setConfirming(true)}
      >
        {label}
      </button>
      {confirming && (
        <ConfirmDialog
          message="Stop renting? Your remote machine will be released."
          confirmLabel="Stop Renting"
          onConfirm={handleConfirm}
          onCancel={() => setConfirming(false)}
        />
      )}
    </>
  );
}
