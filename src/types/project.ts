// Mirrors the Rust `ProjectKind` / `Ide` enums (src-tauri/src/project_kind.rs).
export type ProjectKind = "flutter" | "reactNative" | "android" | "general";
export type Ide = "vscode" | "androidStudio";

export const IDE_LABELS: Record<Ide, string> = {
  vscode: "VS Code",
  androidStudio: "Android Studio",
};

export const PROJECT_KIND_LABELS: Record<ProjectKind, string> = {
  flutter: "Flutter app",
  reactNative: "React Native app",
  android: "Android app",
  general: "General project",
};

// Mobile projects that build for Android get Android Studio (SDK manager,
// emulator images, Gradle tooling); everything else gets VS Code.
export function recommendedIdes(kind: ProjectKind): Ide[] {
  return kind === "general" ? ["vscode"] : ["androidStudio"];
}

// Matches the JSON returned by the Rust `inspect_project_folder` command.
export interface ProjectInfo {
  name: string;
  path: string;
  exists: boolean;
  readable: boolean;
  isEmpty: boolean;
  fileCount: number;
  totalSizeBytes: number;
  kind: ProjectKind;
}

export function isProjectValid(info: ProjectInfo): boolean {
  return info.exists && info.readable && !info.isEmpty;
}
