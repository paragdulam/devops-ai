import {
  recommendedIdes,
  type Ide,
  type ProjectKind,
  type ToolRequirement,
} from "../types/project";
import { RentalStatus, type Rental } from "../types/rental";
import type { GithubRepo } from "../types/github";

export interface SelectedGithubRepo {
  accountId: string;
  repo: GithubRepo;
}

export type SidebarTab = "repos" | "rentals";

// Identifies a repo across GitHub accounts — the key tying a sidebar entry,
// its setup draft and its rental together.
export function repoKey(accountId: string, fullName: string): string {
  return `${accountId}/${fullName}`;
}

export function rentalRepoKey(rental: Rental): string | null {
  return rental.githubRepo
    ? repoKey(rental.githubRepo.githubAccountId, rental.githubRepo.fullName)
    : null;
}

// A rental that has completed creation successfully.
export function isLive(rental: Rental): boolean {
  return rental.status === RentalStatus.READY || rental.status === RentalStatus.RUNNING;
}

// The rental currently attached to a repo: a released one is gone, and when a
// repo has several (a failed attempt followed by a retry) the newest wins.
export function rentalForKey(rentals: Record<string, Rental>, key: string | null): Rental | null {
  if (!key) return null;
  let found: Rental | null = null;
  for (const rental of Object.values(rentals)) {
    if (rental.status === RentalStatus.RELEASED || rentalRepoKey(rental) !== key) continue;
    if (!found || rental.createdAt > found.createdAt) found = rental;
  }
  return found;
}

// The per-repo setup form values, parked while another repo is selected.
interface SetupDraft {
  projectKind: ProjectKind | null;
  selectedIdes: Ide[];
  selectedTools: ToolRequirement[];
}

const EMPTY_DRAFT: SetupDraft = {
  projectKind: null,
  selectedIdes: recommendedIdes("general"),
  selectedTools: [],
};

export interface AppState {
  sidebarTab: SidebarTab;
  githubAccountId: string | null;
  // Which repo/rental the detail pane shows. A repo with no rental shows the
  // setup form, which needs `selectedGithubRepo` to be that same repo.
  selectedRepoKey: string | null;
  selectedGithubRepo: SelectedGithubRepo | null;
  drafts: Record<string, SetupDraft>;
  // Setup form values for `selectedGithubRepo` — seeded from detection, then edited.
  // null while detection is still running.
  projectKind: ProjectKind | null;
  selectedIdes: Ide[];
  selectedTools: ToolRequirement[];
  selectedMachineProfileId: string;
  selectedAccountId: string | null;
  vmUsername: string;
  vmPassword: string;
  rentals: Record<string, Rental>;
  error: string | null;
}

export type AppAction =
  | { type: "SIDEBAR_TAB_CHANGED"; tab: SidebarTab }
  | { type: "GITHUB_ACCOUNT_SELECTED"; accountId: string | null }
  | { type: "GITHUB_REPO_SELECTED"; selection: SelectedGithubRepo }
  | { type: "RENTAL_SELECTED"; rentalId: string }
  | { type: "PROJECT_DETECTED"; kind: ProjectKind | null; tools: ToolRequirement[] }
  | { type: "IDES_CHANGED"; ides: Ide[] }
  | { type: "TOOLS_CHANGED"; tools: ToolRequirement[] }
  | { type: "ACCOUNT_SELECTED"; accountId: string }
  | { type: "VM_CREDENTIALS_CHANGED"; vmUsername: string; vmPassword: string }
  | { type: "RENTALS_LOADED"; rentals: Rental[] }
  | { type: "RENTAL_CREATED"; rental: Rental }
  | { type: "RENTAL_UPDATED"; rental: Rental }
  | { type: "RENTAL_REMOVED"; rentalId: string }
  | { type: "RENTAL_ERROR"; error: string };

export function createInitialState(defaultMachineProfileId: string): AppState {
  return {
    sidebarTab: "repos",
    githubAccountId: null,
    selectedRepoKey: null,
    selectedGithubRepo: null,
    drafts: {},
    projectKind: null,
    selectedIdes: EMPTY_DRAFT.selectedIdes,
    selectedTools: EMPTY_DRAFT.selectedTools,
    selectedMachineProfileId: defaultMachineProfileId,
    selectedAccountId: null,
    vmUsername: "",
    vmPassword: "",
    rentals: {},
    error: null,
  };
}

export function rentalReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "SIDEBAR_TAB_CHANGED":
      return { ...state, sidebarTab: action.tab };
    case "GITHUB_ACCOUNT_SELECTED":
      return { ...state, githubAccountId: action.accountId };
    case "GITHUB_REPO_SELECTED": {
      const { accountId, repo } = action.selection;
      const key = repoKey(accountId, repo.fullName);
      // Park the outgoing repo's form and restore the incoming one's.
      const drafts = state.selectedGithubRepo
        ? {
            ...state.drafts,
            [repoKey(state.selectedGithubRepo.accountId, state.selectedGithubRepo.repo.fullName)]: {
              projectKind: state.projectKind,
              selectedIdes: state.selectedIdes,
              selectedTools: state.selectedTools,
            },
          }
        : state.drafts;
      const draft = drafts[key] ?? EMPTY_DRAFT;
      return {
        ...state,
        drafts,
        selectedRepoKey: key,
        selectedGithubRepo: action.selection,
        ...draft,
        error: null,
      };
    }
    case "RENTAL_SELECTED": {
      const rental = state.rentals[action.rentalId];
      const key = rental ? rentalRepoKey(rental) : null;
      return key ? { ...state, selectedRepoKey: key, error: null } : state;
    }
    case "PROJECT_DETECTED":
      // Re-apply the recommendations each time a new project is detected;
      // the user can still change them afterwards via IDES_CHANGED /
      // TOOLS_CHANGED.
      return {
        ...state,
        projectKind: action.kind,
        selectedIdes: action.kind ? recommendedIdes(action.kind) : state.selectedIdes,
        selectedTools: action.kind ? action.tools : state.selectedTools,
      };
    case "IDES_CHANGED":
      return { ...state, selectedIdes: action.ides };
    case "TOOLS_CHANGED":
      return { ...state, selectedTools: action.tools };
    case "ACCOUNT_SELECTED":
      return { ...state, selectedAccountId: action.accountId, error: null };
    case "VM_CREDENTIALS_CHANGED":
      return { ...state, vmUsername: action.vmUsername, vmPassword: action.vmPassword, error: null };
    case "RENTALS_LOADED": {
      // Merge: a rental already tracked this session is at least as fresh.
      const rentals = { ...state.rentals };
      for (const rental of action.rentals) rentals[rental.id] ??= rental;
      // Launching into restored VMs: open on the Rentals tab.
      const restoredLive =
        Object.keys(state.rentals).length === 0 && action.rentals.some(isLive);
      return { ...state, rentals, sidebarTab: restoredLive ? "rentals" : state.sidebarTab };
    }
    case "RENTAL_CREATED":
      return {
        ...state,
        rentals: { ...state.rentals, [action.rental.id]: action.rental },
        error: null,
      };
    case "RENTAL_UPDATED": {
      const previous = state.rentals[action.rental.id];
      const next = { ...state, rentals: { ...state.rentals, [action.rental.id]: action.rental } };
      // A repo "moves" to Rentals when its rental first completes
      // successfully — but only pull the user along if they're still on it.
      const key = rentalRepoKey(action.rental);
      if (previous && !isLive(previous) && isLive(action.rental) && key === state.selectedRepoKey) {
        next.sidebarTab = "rentals";
      }
      return next;
    }
    case "RENTAL_REMOVED": {
      const { [action.rentalId]: _removed, ...rentals } = state.rentals;
      return { ...state, rentals };
    }
    case "RENTAL_ERROR":
      return { ...state, error: action.error };
    default:
      return state;
  }
}
