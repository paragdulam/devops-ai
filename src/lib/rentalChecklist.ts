import { RentalStatus } from "../types/rental";

export type ChecklistStepState = "done" | "current" | "pending" | "error";

export interface ChecklistStep {
  label: string;
  state: ChecklistStepState;
}

// The 5-step checklist from PRD §8.
const STEPS = [
  "Finding machine",
  "Creating VM",
  "Booting Ubuntu",
  "Connecting to remote machine",
  "Preparing workspace",
] as const;

// Index of the step considered "current" for each in-progress status.
// RUNNING/STOPPING/RELEASED have no checklist left to show — the rental is
// already up (or being torn down) — so every step reads as done.
// FAILED isn't in the PRD's checklist at all; the sane fallback is to flag the
// first step as the error, since the mock can't know which real step failed.
const STATUS_STEP_INDEX: Record<RentalStatus, number | "all-done" | "error"> = {
  [RentalStatus.REQUESTED]: 0,
  [RentalStatus.PROVISIONING]: 1,
  [RentalStatus.BOOTING]: 2,
  [RentalStatus.CONNECTING]: 3,
  [RentalStatus.READY]: 4,
  [RentalStatus.RUNNING]: "all-done",
  [RentalStatus.STOPPING]: "all-done",
  [RentalStatus.RELEASED]: "all-done",
  [RentalStatus.FAILED]: "error",
};

// Ordered phases the Ansible playbook reports via `rental://{id}/step`
// events (see `ansible::phase_for_tags` on the Rust side) — tracked while
// status is PROVISIONING, giving real per-task granularity instead of the
// single guessed "Creating VM" step below.
const PROVISIONING_PHASES = ["user", "desktop", "packages", "clone", "toolchain"] as const;
export type ProvisioningPhase = (typeof PROVISIONING_PHASES)[number];

const PROVISIONING_PHASE_LABELS: Record<ProvisioningPhase, string> = {
  user: "Creating VM login",
  desktop: "Setting up desktop & VNC",
  packages: "Installing packages",
  clone: "Cloning repository",
  toolchain: "Installing project tools (mise)",
};

export function getChecklistSteps(
  status: RentalStatus,
  currentPhase?: string | null,
): ChecklistStep[] {
  if (status === RentalStatus.PROVISIONING && currentPhase) {
    const idx = PROVISIONING_PHASES.indexOf(currentPhase as ProvisioningPhase);
    return PROVISIONING_PHASES.map((phase, i) => {
      const label = PROVISIONING_PHASE_LABELS[phase];
      if (idx === -1) return { label, state: "pending" };
      if (i < idx) return { label, state: "done" };
      if (i === idx) return { label, state: "current" };
      return { label, state: "pending" };
    });
  }

  const marker = STATUS_STEP_INDEX[status];

  if (marker === "all-done") {
    return STEPS.map((label) => ({ label, state: "done" }));
  }

  if (marker === "error") {
    return STEPS.map((label, i) => ({
      label,
      state: i === 0 ? "error" : "pending",
    }));
  }

  return STEPS.map((label, i) => {
    if (i < marker) return { label, state: "done" };
    if (i === marker) return { label, state: "current" };
    return { label, state: "pending" };
  });
}
