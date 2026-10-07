import { useRentalState } from "../state/RentalContext";
import {
  TOOL_LABELS,
  TOOL_SOURCE_LABELS,
  isValidToolVersion,
  type ToolRequirement,
} from "../types/project";

// Lists the tools detected from the project's README (first) and version
// files, for mise to preinstall on the VM. Every row is editable before the
// rental starts.
export function ToolchainPicker() {
  const { state, dispatch } = useRentalState();
  const { selectedGithubRepo, projectKind, selectedTools } = state;

  const hasProject = selectedGithubRepo !== null;
  const addable = Object.keys(TOOL_LABELS).filter(
    (tool) => !selectedTools.some((t) => t.tool === tool),
  );

  function setTools(tools: ToolRequirement[]) {
    dispatch({ type: "TOOLS_CHANGED", tools });
  }

  function setVersion(tool: string, version: string) {
    setTools(selectedTools.map((t) => (t.tool === tool ? { ...t, version, source: "user" } : t)));
  }

  function remove(tool: string) {
    setTools(selectedTools.filter((t) => t.tool !== tool));
  }

  function add(tool: string) {
    if (tool) setTools([...selectedTools, { tool, version: "latest", source: "user" }]);
  }

  if (!hasProject) return null;

  return (
    <section className="toolchain-picker">
      <h2>Preinstalled software</h2>
      <p className="toolchain-picker__note">
        {projectKind
          ? "Detected from the README and version files. Installed with mise after the repo is cloned."
          : "Reading README…"}
      </p>
      {projectKind && selectedTools.length === 0 && (
        <p className="toolchain-picker__note">No tools detected.</p>
      )}
      <ul className="toolchain-picker__list">
        {selectedTools.map((t) => (
          <li key={t.tool} className="toolchain-picker__row">
            <span className="toolchain-picker__tool">{TOOL_LABELS[t.tool] ?? t.tool}</span>
            <input
              type="text"
              aria-label={`${TOOL_LABELS[t.tool] ?? t.tool} version`}
              className={isValidToolVersion(t.version) ? undefined : "toolchain-picker__invalid"}
              value={t.version}
              onChange={(e) => setVersion(t.tool, e.target.value.trim())}
            />
            <span className="toolchain-picker__source">{TOOL_SOURCE_LABELS[t.source]}</span>
            <button type="button" aria-label={`Remove ${t.tool}`} onClick={() => remove(t.tool)}>
              ×
            </button>
          </li>
        ))}
      </ul>
      {projectKind && addable.length > 0 && (
        <select value="" aria-label="Add tool" onChange={(e) => add(e.target.value)}>
          <option value="">+ Add tool…</option>
          {addable.map((tool) => (
            <option key={tool} value={tool}>
              {TOOL_LABELS[tool]}
            </option>
          ))}
        </select>
      )}
    </section>
  );
}
