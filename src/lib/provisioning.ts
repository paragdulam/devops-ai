import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ProvisioningStepEvent {
  phase: string;
  task: string;
  state: string;
}

export function getProvisioningLog(rentalId: string): Promise<string[]> {
  return invoke<string[]>("get_provisioning_log", { id: rentalId });
}

export function onProvisioningLog(
  rentalId: string,
  onLine: (line: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(`rental://${rentalId}/log`, (event) => onLine(event.payload));
}

export function onProvisioningStep(
  rentalId: string,
  onStep: (step: ProvisioningStepEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProvisioningStepEvent>(`rental://${rentalId}/step`, (event) => onStep(event.payload));
}
