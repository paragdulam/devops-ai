import { useEffect } from "react";
import { detectGithubProject } from "../lib/project";
import { useRentalState } from "./RentalContext";

// Detects the picked project's type and toolchain, feeding both the IDE and
// toolchain recommendations. Called once from HomeScreen so the GitHub
// fetch isn't duplicated per picker.
export function useProjectDetection() {
  const { state, dispatch } = useRentalState();
  const { projectSource, projectInfo, selectedGithubRepo } = state;

  useEffect(() => {
    if (projectSource === "local") {
      dispatch({
        type: "PROJECT_DETECTED",
        kind: projectInfo?.kind ?? null,
        tools: projectInfo?.tools ?? [],
      });
      return;
    }

    dispatch({ type: "PROJECT_DETECTED", kind: null, tools: [] });
    if (!selectedGithubRepo) return;

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
  }, [projectSource, projectInfo, selectedGithubRepo, dispatch]);
}
