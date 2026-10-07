import type { Rental } from "../types/rental";

// EC2 bills per second with a 60-second minimum.
const MIN_BILLED_MS = 60_000;

// Running cost estimate: hourly rate x time since the instance launched,
// frozen at `stoppedAt` once released. null until the instance has launched
// and its rate is known.
export function estimateCostUsd(rental: Rental, nowMs: number): number | null {
  if (!rental.launchedAt || rental.hourlyRateUsd == null) return null;
  const start = new Date(rental.launchedAt).getTime();
  const end = rental.stoppedAt ? new Date(rental.stoppedAt).getTime() : nowMs;
  const billedMs = Math.max(MIN_BILLED_MS, end - start);
  return (billedMs / 3_600_000) * rental.hourlyRateUsd;
}

export function formatUsd(amount: number): string {
  return `$${amount.toFixed(amount < 0.1 ? 3 : 2)}`;
}
