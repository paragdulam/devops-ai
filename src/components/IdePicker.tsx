import { useRentalState } from "../state/RentalContext";
import { IDE_LABELS, PROJECT_KIND_LABELS, recommendedIdes, type Ide } from "../types/project";

const ALL_IDES: Ide[] = ["vscode", "androidStudio"];

// Preselects the recommended IDE for the detected project type (see
// useProjectDetection); the user can override the choice before starting.
export function IdePicker() {
  const { state, dispatch } = useRentalState();
  const { projectSource, projectInfo, selectedGithubRepo, projectKind, selectedIdes } = state;

  function toggle(ide: Ide, checked: boolean) {
    const ides = checked
      ? ALL_IDES.filter((i) => i === ide || selectedIdes.includes(i))
      : selectedIdes.filter((i) => i !== ide);
    dispatch({ type: "IDES_CHANGED", ides });
  }

  const hasProject = projectSource === "local" ? projectInfo !== null : selectedGithubRepo !== null;
  const recommended = projectKind ? recommendedIdes(projectKind) : [];

  return (
    <section className="ide-picker">
      <h2>IDE</h2>
      {hasProject && (
        <p className="ide-picker__detected">
          {projectKind
            ? `Detected: ${PROJECT_KIND_LABELS[projectKind]}`
            : "Detecting project type…"}
        </p>
      )}
      <div className="ide-picker__options">
        {ALL_IDES.map((ide) => (
          <label key={ide}>
            <input
              type="checkbox"
              checked={selectedIdes.includes(ide)}
              onChange={(e) => toggle(ide, e.target.checked)}
            />{" "}
            {IDE_LABELS[ide]}
            {recommended.includes(ide) && " (recommended)"}
          </label>
        ))}
      </div>
    </section>
  );
}
