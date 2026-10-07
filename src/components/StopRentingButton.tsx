import { useState } from "react";
import { useRentalState } from "../state/RentalContext";
import { ConfirmDialog } from "./ConfirmDialog";

export function StopRentingButton({
  rentalId,
  label = "STOP RENTING",
}: {
  rentalId: string;
  label?: string;
}) {
  const { dispatch, rentalService } = useRentalState();
  const [confirming, setConfirming] = useState(false);
  const [stopping, setStopping] = useState(false);

  async function handleConfirm() {
    setConfirming(false);
    setStopping(true);
    try {
      await rentalService.stopRental(rentalId);
      // useRentalSync's polling carries the STOPPING -> RELEASED transition.
    } catch (err) {
      setStopping(false);
      dispatch({
        type: "RENTAL_ERROR",
        error: err instanceof Error ? err.message : String(err),
      });
    }
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
