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

// Mirrors the Rust `ToolSource` / `ToolRequirement` (src-tauri/src/toolchain.rs).
export type ToolSource =
  | "readme"
  | "toolVersions"
  | "miseToml"
  | "nvmrc"
  | "nodeVersion"
  | "pythonVersion"
  | "rubyVersion"
  | "javaVersion"
  | "goMod"
  | "rustToolchain"
  | "packageJson"
  | "pubspec"
  | "kindDefault"
  | "user";

// A tool mise installs on the VM: `mise use --global {tool}@{version}`.
export interface ToolRequirement {
  tool: string;
  version: string;
  source: ToolSource;
}

// Keys are the Rust allowlist (`toolchain::TOOLS`) — anything else is
// rejected by `start_rental`.
export const TOOL_LABELS: Record<string, string> = {
  node: "Node.js",
  python: "Python",
  java: "Java",
  go: "Go",
  rust: "Rust",
  ruby: "Ruby",
  flutter: "Flutter",
  gradle: "Gradle",
  maven: "Maven",
  pnpm: "pnpm",
  yarn: "Yarn",
  bun: "Bun",
  deno: "Deno",
  terraform: "Terraform",
  kubectl: "kubectl",
  helm: "Helm",
  "aws-cli": "AWS CLI",
};

export const TOOL_SOURCE_LABELS: Record<ToolSource, string> = {
  readme: "README",
  toolVersions: ".tool-versions",
  miseToml: "mise.toml",
  nvmrc: ".nvmrc",
  nodeVersion: ".node-version",
  pythonVersion: ".python-version",
  rubyVersion: ".ruby-version",
  javaVersion: ".java-version",
  goMod: "go.mod",
  rustToolchain: "rust-toolchain",
  packageJson: "package.json",
  pubspec: "pubspec.yaml",
  kindDefault: "project type",
  user: "added",
};

// Same rule as the Rust `toolchain::validate`, checked up front so a bad
// version is flagged in the form rather than rejected by `start_rental`.
export function isValidToolVersion(version: string): boolean {
  return /^[A-Za-z0-9._+-]{1,40}$/.test(version);
}

// Matches the JSON returned by the Rust `detect_github_project` command.
export interface ProjectDetection {
  kind: ProjectKind;
  tools: ToolRequirement[];
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
  tools: ToolRequirement[];
}

export function isProjectValid(info: ProjectInfo): boolean {
  return info.exists && info.readable && !info.isEmpty;
}
