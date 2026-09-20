import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MockRentalService, STEP_DELAY_MS } from "./mockRentalService";
import { RentalStatus } from "../types/rental";

describe("MockRentalService", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("progresses through the full ordered lifecycle with no skipped states", async () => {
    const service = new MockRentalService();
    const rental = await service.createRental({
      accountId: "acct-1",
      machineProfile: "standard",
      projectName: "my-app",
      vmUsername: "dev",
      vmPassword: "correct-horse-battery-staple",
    });
    expect(rental.status).toBe(RentalStatus.REQUESTED);

    const seen: RentalStatus[] = [];
    service.subscribeToRental(rental.id, (updated) => seen.push(updated.status));

    await vi.advanceTimersByTimeAsync(STEP_DELAY_MS * 5);

    expect(seen).toEqual([
      RentalStatus.PROVISIONING,
      RentalStatus.BOOTING,
      RentalStatus.CONNECTING,
      RentalStatus.READY,
      RentalStatus.RUNNING,
    ]);

    const finalRental = await service.getRental(rental.id);
    expect(finalRental.status).toBe(RentalStatus.RUNNING);
    expect(finalRental.ec2InstanceId).not.toBeNull();
    expect(finalRental.startedAt).not.toBeNull();
  });

  it("drives STOPPING then RELEASED on stopRental", async () => {
    const service = new MockRentalService();
    const rental = await service.createRental({
      accountId: "acct-1",
      machineProfile: "standard",
      projectName: "my-app",
      vmUsername: "dev",
      vmPassword: "correct-horse-battery-staple",
    });
    await vi.advanceTimersByTimeAsync(STEP_DELAY_MS * 5); // reach RUNNING

    const seen: RentalStatus[] = [];
    service.subscribeToRental(rental.id, (updated) => seen.push(updated.status));

    const stopped = await service.stopRental(rental.id);
    expect(stopped.status).toBe(RentalStatus.STOPPING);

    await vi.advanceTimersByTimeAsync(STEP_DELAY_MS);

    expect(seen).toEqual([RentalStatus.STOPPING, RentalStatus.RELEASED]);
    const finalRental = await service.getRental(rental.id);
    expect(finalRental.stoppedAt).not.toBeNull();
  });

  it("reflects the latest state at any point via getRental", async () => {
    const service = new MockRentalService();
    const rental = await service.createRental({
      accountId: "acct-1",
      machineProfile: "standard",
      projectName: "my-app",
      vmUsername: "dev",
      vmPassword: "correct-horse-battery-staple",
    });

    await vi.advanceTimersByTimeAsync(STEP_DELAY_MS * 2);
    const midRental = await service.getRental(rental.id);
    expect(midRental.status).toBe(RentalStatus.BOOTING);
  });
});
