import { useEffect, useRef } from "react";
import { detectGithubProject } from "../lib/github";
import { useRentalState } from "./RentalContext";

// Detects the selected repo's type and toolchain, feeding both the IDE and
// toolchain recommendations. Called once from WorkspaceScreen so the GitHub
// fetch isn't duplicated per picker. A repo whose draft already holds a
// detection result (re-selected from the sidebar) is not fetched again.
export function useProjectDetection() {
  const { state, dispatch } = useRentalState();
  const { selectedGithubRepo, projectKind } = state;
  const detectedRef = useRef(projectKind !== null);
  detectedRef.current = projectKind !== null;

  useEffect(() => {
    if (!selectedGithubRepo || detectedRef.current) return;

    let cancelled = false;
    const { accountId, repo } = selectedGithubRepo;
    detectGithubProject(accountId, repo.fullName, repo.defaultBranch)
      .catch(() => ({ kind: "general" as const, tools: [] }))
      .then(({ kind, tools }) => {
        if (!cancelled) dispatch({ type: "PROJECT_DETECTED", kind, tools });
      });
    return () => {
      cancelled = true;
    };
  }, [selectedGithubRepo, dispatch]);
}
