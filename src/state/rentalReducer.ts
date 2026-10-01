import { recommendedIdes, type Ide, type ProjectInfo, type ProjectKind } from "../types/project";
import type { Rental } from "../types/rental";
import type { GithubRepo } from "../types/github";

export type ProjectSource = "local" | "github";

export interface SelectedGithubRepo {
  accountId: string;
  repo: GithubRepo;
}

// A single active rental only — no history, no concurrency, matching the
// PRD's single-rental-at-a-time MVP scope.
export interface AppState {
  projectSource: ProjectSource;
  projectInfo: ProjectInfo | null;
  selectedGithubRepo: SelectedGithubRepo | null;
  // null while no project is picked or detection is still running.
  projectKind: ProjectKind | null;
  selectedIdes: Ide[];
  selectedMachineProfileId: string;
  selectedAccountId: string | null;
  vmUsername: string;
  vmPassword: string;
  rental: Rental | null;
  error: string | null;
}

export type AppAction =
  | { type: "PROJECT_SOURCE_CHANGED"; source: ProjectSource }
  | { type: "PROJECT_SELECTED"; projectInfo: ProjectInfo | null }
  | { type: "GITHUB_REPO_SELECTED"; selection: SelectedGithubRepo | null }
  | { type: "PROJECT_KIND_DETECTED"; kind: ProjectKind | null }
  | { type: "IDES_CHANGED"; ides: Ide[] }
  | { type: "ACCOUNT_SELECTED"; accountId: string }
  | { type: "VM_CREDENTIALS_CHANGED"; vmUsername: string; vmPassword: string }
  | { type: "RENTAL_CREATED"; rental: Rental }
  | { type: "RENTAL_UPDATED"; rental: Rental }
  | { type: "RENTAL_ERROR"; error: string }
  | { type: "RESET" };

export function createInitialState(defaultMachineProfileId: string): AppState {
  return {
    projectSource: "local",
    projectInfo: null,
    selectedGithubRepo: null,
    projectKind: null,
    selectedIdes: recommendedIdes("general"),
    selectedMachineProfileId: defaultMachineProfileId,
    selectedAccountId: null,
    vmUsername: "",
    vmPassword: "",
    rental: null,
    error: null,
  };
}

export function rentalReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "PROJECT_SOURCE_CHANGED":
      return {
        ...state,
        projectSource: action.source,
        projectInfo: null,
        selectedGithubRepo: null,
        projectKind: null,
        error: null,
      };
    case "PROJECT_SELECTED":
      return { ...state, projectInfo: action.projectInfo, error: null };
    case "GITHUB_REPO_SELECTED":
      return { ...state, selectedGithubRepo: action.selection, error: null };
    case "PROJECT_KIND_DETECTED":
      // Re-apply the recommendation each time a new project is detected;
      // the user can still change it afterwards via IDES_CHANGED.
      return {
        ...state,
        projectKind: action.kind,
        selectedIdes: action.kind ? recommendedIdes(action.kind) : state.selectedIdes,
      };
    case "IDES_CHANGED":
      return { ...state, selectedIdes: action.ides };
    case "ACCOUNT_SELECTED":
      return { ...state, selectedAccountId: action.accountId, error: null };
    case "VM_CREDENTIALS_CHANGED":
      return { ...state, vmUsername: action.vmUsername, vmPassword: action.vmPassword, error: null };
    case "RENTAL_CREATED":
      return { ...state, rental: action.rental, error: null };
    case "RENTAL_UPDATED":
      return { ...state, rental: action.rental };
    case "RENTAL_ERROR":
      return { ...state, error: action.error };
    case "RESET":
      return {
        ...state,
        rental: null,
        error: null,
      };
    default:
      return state;
  }
}
