// Mirrors the backend's rental lifecycle exactly (PRD §8) and the Rental record
// shape (PRD §19), so the mock service in Milestone 1 needs no translation layer
// once Milestone 2 wires up the real HTTP/WebSocket backend.

export enum RentalStatus {
  REQUESTED = "REQUESTED",
  PROVISIONING = "PROVISIONING",
  BOOTING = "BOOTING",
  CONNECTING = "CONNECTING",
  READY = "READY",
  RUNNING = "RUNNING",
  STOPPING = "STOPPING",
  RELEASED = "RELEASED",
  FAILED = "FAILED",
}

export interface RentalConnection {
  publicIp: string;
  vncPort: number;
  vncPassword: string;
}

export interface Rental {
  id: string;
  status: RentalStatus;
  ec2InstanceId: string | null;
  machineProfile: string;
  projectName: string;
  createdAt: string; // ISO 8601
  startedAt: string | null;
  stoppedAt: string | null;
  error?: string;
  connection: RentalConnection | null;
}

// Mirrors the Rust `GithubRepoSelection` struct — sent to `start_rental` when
// the chosen project came from GitHub rather than a local folder.
export interface GithubRepoSelectionRequest {
  githubAccountId: string;
  repoName: string;
  fullName: string;
  cloneUrl: string;
  defaultBranch: string;
}

import type { Ide, ToolRequirement } from "./project";

export interface CreateRentalRequest {
  accountId: string;
  machineProfile: string;
  projectName: string;
  vmUsername: string;
  vmPassword: string;
  githubRepo?: GithubRepoSelectionRequest;
  ides: Ide[];
  tools: ToolRequirement[];
}

export type CreateRentalResponse = Rental;
