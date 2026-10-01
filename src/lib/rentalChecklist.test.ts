import { describe, expect, it } from "vitest";
import { getChecklistSteps } from "./rentalChecklist";
import { RentalStatus } from "../types/rental";

describe("getChecklistSteps", () => {
  it("marks earlier steps done and the current step as current", () => {
    const steps = getChecklistSteps(RentalStatus.CONNECTING);
    expect(steps.map((s) => s.state)).toEqual([
      "done",
      "done",
      "done",
      "current",
      "pending",
    ]);
  });

  it("starts with only the first step current for REQUESTED", () => {
    const steps = getChecklistSteps(RentalStatus.REQUESTED);
    expect(steps.map((s) => s.state)).toEqual([
      "current",
      "pending",
      "pending",
      "pending",
      "pending",
    ]);
  });

  it("marks every step done once RUNNING, STOPPING, or RELEASED", () => {
    for (const status of [
      RentalStatus.RUNNING,
      RentalStatus.STOPPING,
      RentalStatus.RELEASED,
    ]) {
      const steps = getChecklistSteps(status);
      expect(steps.every((s) => s.state === "done")).toBe(true);
    }
  });

  it("flags the first step as an error fallback for FAILED", () => {
    const steps = getChecklistSteps(RentalStatus.FAILED);
    expect(steps[0].state).toBe("error");
    expect(steps.slice(1).every((s) => s.state === "pending")).toBe(true);
  });

  it("falls back to the guessed static steps for PROVISIONING with no phase yet", () => {
    const steps = getChecklistSteps(RentalStatus.PROVISIONING);
    expect(steps.map((s) => s.state)).toEqual(["done", "current", "pending", "pending", "pending"]);
  });

  it("drives real per-task phases once Ansible events arrive during PROVISIONING", () => {
    const steps = getChecklistSteps(RentalStatus.PROVISIONING, "desktop");
    expect(steps.map((s) => s.state)).toEqual(["done", "current", "pending", "pending", "pending"]);
    expect(steps.map((s) => s.label)).toEqual([
      "Creating VM login",
      "Setting up desktop & VNC",
      "Installing packages",
      "Cloning repository",
      "Installing project tools (mise)",
    ]);
  });

  it("marks earlier provisioning phases done while cloning", () => {
    const steps = getChecklistSteps(RentalStatus.PROVISIONING, "clone");
    expect(steps.map((s) => s.state)).toEqual(["done", "done", "done", "current", "pending"]);
  });

  it("tracks the mise toolchain phase last", () => {
    const steps = getChecklistSteps(RentalStatus.PROVISIONING, "toolchain");
    expect(steps.map((s) => s.state)).toEqual(["done", "done", "done", "done", "current"]);
  });
});
