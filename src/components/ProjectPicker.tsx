import { useState } from "react";
import { pickProjectFolder, inspectProjectFolder } from "../lib/project";
import { useRentalState } from "../state/RentalContext";
import { ProjectSummary } from "./ProjectSummary";
import { GithubAccountPicker } from "./GithubAccountPicker";
import { RepoPicker } from "./RepoPicker";

export function ProjectPicker() {
  const { state, dispatch } = useRentalState();
  const [pickerError, setPickerError] = useState<string | null>(null);
  const [githubAccountId, setGithubAccountId] = useState<string | null>(null);

  async function handleChooseFolder() {
    setPickerError(null);
    try {
      const path = await pickProjectFolder();
      if (!path) return;
      const info = await inspectProjectFolder(path);
      dispatch({ type: "PROJECT_SELECTED", projectInfo: info });
    } catch (err) {
      setPickerError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <section className="project-picker">
      <h2>Project</h2>
      <div className="project-picker__source-toggle">
        <button
          type="button"
          className={state.projectSource === "local" ? "active" : ""}
          onClick={() => dispatch({ type: "PROJECT_SOURCE_CHANGED", source: "local" })}
        >
          Local Folder
        </button>
        <button
          type="button"
          className={state.projectSource === "github" ? "active" : ""}
          onClick={() => dispatch({ type: "PROJECT_SOURCE_CHANGED", source: "github" })}
        >
          GitHub Repo
        </button>
      </div>

      {state.projectSource === "local" ? (
        <>
          {state.projectInfo ? (
            <ProjectSummary info={state.projectInfo} />
          ) : (
            <p className="project-picker__placeholder">No folder selected.</p>
          )}
          {pickerError && <p className="validation-error">{pickerError}</p>}
          <button type="button" onClick={handleChooseFolder}>
            Choose Folder
          </button>
        </>
      ) : (
        <>
          <GithubAccountPicker selectedAccountId={githubAccountId} onSelect={setGithubAccountId} />
          <RepoPicker accountId={githubAccountId} />
        </>
      )}
    </section>
  );
}
