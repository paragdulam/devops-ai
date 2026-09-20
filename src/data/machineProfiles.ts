export interface MachineProfile {
  id: string;
  label: string;
  cpu: number;
  ramGb: number;
  provider: string;
}

// Hardcoded for the MVP (PRD §5.1, §9). `id: "standard"` matches the literal
// value in the PRD §7 example `POST /rentals` body, so it flows unchanged
// into the real request once Milestone 2 lands.
export const MACHINE_PROFILES: MachineProfile[] = [
  {
    id: "standard",
    label: "AWS EC2 — 8 CPU / 32 GB RAM",
    cpu: 8,
    ramGb: 32,
    provider: "aws-ec2",
  },
];
