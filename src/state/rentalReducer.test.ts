import { describe, expect, it } from "vitest";
import {
  createInitialState,
  rentalForKey,
  rentalReducer,
  repoKey,
  type AppState,
} from "./rentalReducer";
import { RentalStatus, type Rental } from "../types/rental";
import type { GithubRepo } from "../types/github";

const rental: Rental = {
  id: "rental-1",
  status: RentalStatus.REQUESTED,
  ec2InstanceId: null,
  machineProfile: "standard",
  projectName: "my-app",
  githubRepo: {
    githubAccountId: "gh-1",
    repoName: "my-app",
    fullName: "acme/my-app",
    cloneUrl: "https://github.com/acme/my-app.git",
    defaultBranch: "main",
  },
  createdAt: "2026-01-01T00:00:00.000Z",
  startedAt: null,
  stoppedAt: null,
  connection: null,
};

const repo = (name: string): GithubRepo => ({
  id: name.length,
  name,
  fullName: `acme/${name}`,
  private: false,
  cloneUrl: `https://github.com/acme/${name}.git`,
  defaultBranch: "main",
});

const initial = () => createInitialState("standard");

function withRental(state: AppState, r: Rental): AppState {
  return rentalReducer(state, { type: "RENTAL_CREATED", rental: r });
}

describe("rentals", () => {
  it("tracks several rentals at once", () => {
    let state = withRental(initial(), rental);
    state = withRental(state, { ...rental, id: "rental-2" });
    expect(Object.keys(state.rentals).sort()).toEqual(["rental-1", "rental-2"]);
  });

  it("applies updates to the matching rental only", () => {
    let state = withRental(initial(), rental);
    state = withRental(state, { ...rental, id: "rental-2" });
    state = rentalReducer(state, {
      type: "RENTAL_UPDATED",
      rental: { ...rental, status: RentalStatus.RUNNING },
    });
    expect(state.rentals["rental-1"].status).toBe(RentalStatus.RUNNING);
    expect(state.rentals["rental-2"].status).toBe(RentalStatus.REQUESTED);
  });

  it("removes a rental", () => {
    const state = rentalReducer(withRental(initial(), rental), {
      type: "RENTAL_REMOVED",
      rentalId: "rental-1",
    });
    expect(state.rentals).toEqual({});
  });

  it("hydrates restored rentals without overwriting fresher ones", () => {
    const running = { ...rental, status: RentalStatus.RUNNING };
    const state = rentalReducer(withRental(initial(), running), {
      type: "RENTALS_LOADED",
      rentals: [rental, { ...rental, id: "rental-2", status: RentalStatus.RUNNING }],
    });
    expect(state.rentals["rental-1"].status).toBe(RentalStatus.RUNNING);
    expect(state.rentals["rental-2"]).toBeDefined();
  });

  it("opens on the Rentals tab when launching into restored live rentals", () => {
    const state = rentalReducer(initial(), {
      type: "RENTALS_LOADED",
      rentals: [{ ...rental, status: RentalStatus.RUNNING }],
    });
    expect(state.sidebarTab).toBe("rentals");
  });

  it("finds a repo's current rental, ignoring released ones", () => {
    const key = repoKey("gh-1", "acme/my-app");
    let state = withRental(initial(), { ...rental, status: RentalStatus.RELEASED });
    expect(rentalForKey(state.rentals, key)).toBeNull();
    state = withRental(state, { ...rental, id: "rental-2", createdAt: "2026-02-01T00:00:00.000Z" });
    expect(rentalForKey(state.rentals, key)?.id).toBe("rental-2");
    expect(rentalForKey(state.rentals, null)).toBeNull();
  });
});

describe("moving a repo to Rentals", () => {
  const key = repoKey("gh-1", "acme/my-app");
  const provisioning = { ...rental, status: RentalStatus.PROVISIONING };
  const running = { ...rental, status: RentalStatus.RUNNING };

  function selectedAndProvisioning(): AppState {
    let state = rentalReducer(initial(), {
      type: "GITHUB_REPO_SELECTED",
      selection: { accountId: "gh-1", repo: repo("my-app") },
    });
    state = withRental(state, provisioning);
    return state;
  }

  it("switches to the Rentals tab when the selected repo's rental first goes live", () => {
    const state = rentalReducer(selectedAndProvisioning(), {
      type: "RENTAL_UPDATED",
      rental: running,
    });
    expect(state.selectedRepoKey).toBe(key);
    expect(state.sidebarTab).toBe("rentals");
  });

  it("leaves the tab alone if the user moved to another repo meanwhile", () => {
    let state = selectedAndProvisioning();
    state = rentalReducer(state, {
      type: "GITHUB_REPO_SELECTED",
      selection: { accountId: "gh-1", repo: repo("other") },
    });
    state = rentalReducer(state, { type: "RENTAL_UPDATED", rental: running });
    expect(state.sidebarTab).toBe("repos");
  });

  it("does not switch again on later updates of an already-live rental", () => {
    let state = rentalReducer(selectedAndProvisioning(), {
      type: "RENTAL_UPDATED",
      rental: running,
    });
    state = rentalReducer(state, { type: "SIDEBAR_TAB_CHANGED", tab: "repos" });
    state = rentalReducer(state, { type: "RENTAL_UPDATED", rental: running });
    expect(state.sidebarTab).toBe("repos");
  });

  it("selects a rental's repo from the Rentals tab", () => {
    const state = rentalReducer(withRental(initial(), running), {
      type: "RENTAL_SELECTED",
      rentalId: "rental-1",
    });
    expect(state.selectedRepoKey).toBe(key);
  });
});

describe("per-repo setup drafts", () => {
  const node = { tool: "node", version: "20", source: "readme" } as const;

  it("restores a repo's edits when it is selected again", () => {
    let state = rentalReducer(initial(), {
      type: "GITHUB_REPO_SELECTED",
      selection: { accountId: "gh-1", repo: repo("app-a") },
    });
    state = rentalReducer(state, { type: "PROJECT_DETECTED", kind: "android", tools: [node] });
    state = rentalReducer(state, { type: "IDES_CHANGED", ides: ["vscode", "androidStudio"] });

    state = rentalReducer(state, {
      type: "GITHUB_REPO_SELECTED",
      selection: { accountId: "gh-1", repo: repo("app-b") },
    });
    expect(state.projectKind).toBeNull();
    expect(state.selectedTools).toEqual([]);

    state = rentalReducer(state, {
      type: "GITHUB_REPO_SELECTED",
      selection: { accountId: "gh-1", repo: repo("app-a") },
    });
    expect(state.projectKind).toBe("android");
    expect(state.selectedTools).toEqual([node]);
    expect(state.selectedIdes).toEqual(["vscode", "androidStudio"]);
  });
});

describe("IDE recommendation", () => {
  it("defaults to VS Code before any project is detected", () => {
    expect(initial().selectedIdes).toEqual(["vscode"]);
  });

  it.each([
    ["android", ["androidStudio"]],
    ["flutter", ["androidStudio"]],
    ["reactNative", ["androidStudio"]],
    ["general", ["vscode"]],
  ] as const)("recommends %s -> %j", (kind, ides) => {
    const next = rentalReducer(initial(), {
      type: "PROJECT_DETECTED",
      kind,
      tools: [],
    });
    expect(next.projectKind).toBe(kind);
    expect(next.selectedIdes).toEqual(ides);
  });

  it("keeps a manual override until a new project is detected", () => {
    let state = rentalReducer(initial(), {
      type: "PROJECT_DETECTED",
      kind: "android",
      tools: [],
    });
    state = rentalReducer(state, { type: "IDES_CHANGED", ides: ["vscode", "androidStudio"] });
    expect(state.selectedIdes).toEqual(["vscode", "androidStudio"]);

    state = rentalReducer(state, { type: "PROJECT_DETECTED", kind: null, tools: [] });
    expect(state.selectedIdes).toEqual(["vscode", "androidStudio"]);
  });
});

describe("toolchain", () => {
  const node20 = { tool: "node", version: "20", source: "readme" } as const;
  const java17 = { tool: "java", version: "temurin-17", source: "kindDefault" } as const;

  it("seeds the tools from detection", () => {
    const state = rentalReducer(initial(), {
      type: "PROJECT_DETECTED",
      kind: "reactNative",
      tools: [node20, java17],
    });
    expect(state.selectedTools).toEqual([node20, java17]);
  });

  it("keeps edits while detection is pending, replaces them for a new project", () => {
    let state = rentalReducer(initial(), {
      type: "PROJECT_DETECTED",
      kind: "general",
      tools: [node20],
    });
    const edited = [{ ...node20, version: "22", source: "user" as const }];
    state = rentalReducer(state, { type: "TOOLS_CHANGED", tools: edited });
    expect(state.selectedTools).toEqual(edited);

    state = rentalReducer(state, { type: "PROJECT_DETECTED", kind: null, tools: [] });
    expect(state.selectedTools).toEqual(edited);

    state = rentalReducer(state, { type: "PROJECT_DETECTED", kind: "android", tools: [java17] });
    expect(state.selectedTools).toEqual([java17]);
  });
});
