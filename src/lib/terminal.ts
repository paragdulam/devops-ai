import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function openSshTerminal(rentalId: string): Promise<string> {
  return invoke<string>("open_ssh_terminal", { rentalId });
}

export function writeTerminal(sessionId: string, data: string): Promise<void> {
  return invoke("write_terminal", { sessionId, data });
}

export function resizeTerminal(sessionId: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_terminal", { sessionId, cols, rows });
}

export function closeTerminal(sessionId: string): Promise<void> {
  return invoke("close_terminal", { sessionId });
}

export function onTerminalData(
  sessionId: string,
  onData: (chunk: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(`ssh-terminal://${sessionId}/data`, (event) => onData(event.payload));
}
