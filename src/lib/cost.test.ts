import { describe, expect, it } from "vitest";
import { estimateCostUsd, formatUsd } from "./cost";
import { RentalStatus, type Rental } from "../types/rental";

const base: Rental = {
  id: "r1",
  status: RentalStatus.RUNNING,
  ec2InstanceId: "i-1",
  machineProfile: "standard",
  projectName: "app",
  createdAt: "2026-01-01T00:00:00.000Z",
  startedAt: null,
  stoppedAt: null,
  launchedAt: "2026-01-01T00:00:00.000Z",
  hourlyRateUsd: 0.4,
  connection: null,
};
const HOUR = 3_600_000;
const t0 = new Date(base.launchedAt!).getTime();

describe("estimateCostUsd", () => {
  it("is rate x elapsed hours while running", () => {
    expect(estimateCostUsd(base, t0 + 2 * HOUR)).toBeCloseTo(0.8);
  });

  it("freezes at stoppedAt once released", () => {
    const stopped = { ...base, stoppedAt: new Date(t0 + HOUR).toISOString() };
    expect(estimateCostUsd(stopped, t0 + 10 * HOUR)).toBeCloseTo(0.4);
  });

  it("bills a 60 second minimum", () => {
    expect(estimateCostUsd(base, t0 + 5_000)).toBeCloseTo(0.4 / 60);
  });

  it("is null before launch or without a rate", () => {
    expect(estimateCostUsd({ ...base, launchedAt: null }, t0)).toBeNull();
    expect(estimateCostUsd({ ...base, hourlyRateUsd: null }, t0)).toBeNull();
    expect(estimateCostUsd({ ...base, hourlyRateUsd: undefined }, t0)).toBeNull();
  });
});

describe("formatUsd", () => {
  it("uses three decimals under 10 cents", () => {
    expect(formatUsd(0.0067)).toBe("$0.007");
  });
  it("uses two decimals otherwise", () => {
    expect(formatUsd(1.2345)).toBe("$1.23");
  });
});
