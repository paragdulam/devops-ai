import { useEffect, useRef } from "react";
import { RentalStatus } from "../types/rental";
import { useRentalState } from "./RentalContext";

const isSettled = (status: RentalStatus) =>
  status === RentalStatus.RELEASED || status === RentalStatus.FAILED;

// Keeps every rental's state fresh, not just the one on screen: hydrates the
// rentals the backend restored from disk at launch, then polls each rental
// until it settles (RELEASED / FAILED).
export function useRentalSync() {
  const { state, dispatch, rentalService } = useRentalState();
  const subscriptions = useRef(new Map<string, () => void>());

  useEffect(() => {
    rentalService
      .listRentals()
      .then((rentals) => dispatch({ type: "RENTALS_LOADED", rentals }))
      .catch(() => {
        // Nothing to restore, or the backend isn't reachable yet.
      });
  }, [rentalService, dispatch]);

  const activeIds = Object.values(state.rentals)
    .filter((r) => !isSettled(r.status))
    .map((r) => r.id)
    .sort()
    .join(",");

  useEffect(() => {
    const wanted = new Set(activeIds ? activeIds.split(",") : []);
    for (const [id, unsubscribe] of subscriptions.current) {
      if (!wanted.has(id)) {
        unsubscribe();
        subscriptions.current.delete(id);
      }
    }
    for (const id of wanted) {
      if (subscriptions.current.has(id)) continue;
      subscriptions.current.set(
        id,
        rentalService.subscribeToRental(id, (rental) =>
          dispatch({ type: "RENTAL_UPDATED", rental }),
        ),
      );
    }
  }, [activeIds, rentalService, dispatch]);

  useEffect(
    () => () => {
      subscriptions.current.forEach((unsubscribe) => unsubscribe());
      subscriptions.current.clear();
    },
    [],
  );
}
