import { useState } from "react";
import { estimateCostUsd, formatUsd } from "../lib/cost";
import { useNow } from "../state/useNow";
import { useRentalState } from "../state/RentalContext";
import type { ActualCost, Rental } from "../types/rental";

const ESTIMATE_NOTE =
  "On-demand list price for the instance, disk and public IP. Excludes taxes, discounts and data transfer.";

// Live estimate. `compact` is the sidebar variant (cost only).
export function RentalCostEstimate({ rental, compact }: { rental: Rental; compact?: boolean }) {
  const now = useNow();
  const cost = estimateCostUsd(rental, now);
  if (cost === null) return null;

  if (compact) {
    return (
      <span className="rental-cost rental-cost--compact" title={ESTIMATE_NOTE}>
        ~{formatUsd(cost)}
      </span>
    );
  }
  return (
    <span className="rental-cost" title={ESTIMATE_NOTE}>
      ~{formatUsd(cost)} so far · {formatUsd(rental.hourlyRateUsd!)}/hr
      {rental.rateSource === "fallback" && " (approx. rate)"}
    </span>
  );
}

// Billed cost from AWS Cost Explorer. Fetched only on click — each request
// costs $0.01 and the data lags about a day.
export function RentalActualCost({ rental }: { rental: Rental }) {
  const { rentalService } = useRentalState();
  const [actual, setActual] = useState<ActualCost | null>(null);
  const [loading, setLoading] = useState(false);

  async function refresh() {
    setLoading(true);
    try {
      // A repeat click is an explicit ask for fresh data, so skip the cache.
      setActual(await rentalService.getActualCost(rental.id, actual !== null));
    } catch (err) {
      setActual({ state: "unavailable", reason: err instanceof Error ? err.message : String(err) });
    } finally {
      setLoading(false);
    }
  }

  return (
    <span className="rental-cost rental-cost--actual">
      Actual (AWS):{" "}
      {actual?.state === "available" ? (
        <>
          {formatUsd(actual.amountUsd)} through {actual.throughDate}
        </>
      ) : actual ? (
        <span className="rental-cost__hint">{actual.reason}</span>
      ) : (
        "—"
      )}{" "}
      <button type="button" disabled={loading} onClick={refresh}>
        {loading ? "Checking…" : actual ? "Refresh" : "Check"}
      </button>
    </span>
  );
}
