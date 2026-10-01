import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { ProjectInfo, ProjectKind } from "../types/project";

export async function pickProjectFolder(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  if (Array.isArray(selected)) return selected[0] ?? null;
  return selected ?? null;
}

export async function inspectProjectFolder(path: string): Promise<ProjectInfo> {
  return invoke<ProjectInfo>("inspect_project_folder", { path });
}

export async function detectGithubProjectKind(
  accountId: string,
  fullName: string,
  branch: string,
): Promise<ProjectKind> {
  return invoke<ProjectKind>("detect_github_project_kind", { accountId, fullName, branch });
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unitIndex]}`;
}
