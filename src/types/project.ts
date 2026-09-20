// Matches the JSON returned by the Rust `inspect_project_folder` command.
export interface ProjectInfo {
  name: string;
  path: string;
  exists: boolean;
  readable: boolean;
  isEmpty: boolean;
  fileCount: number;
  totalSizeBytes: number;
}

export function isProjectValid(info: ProjectInfo): boolean {
  return info.exists && info.readable && !info.isEmpty;
}
