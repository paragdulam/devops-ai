import type { ProjectInfo } from "../types/project";
import { formatSize } from "../lib/project";

export function ProjectSummary({ info }: { info: ProjectInfo }) {
  if (!info.exists) {
    return <p className="validation-error">Folder not found.</p>;
  }
  if (!info.readable) {
    return <p className="validation-error">Folder is not readable.</p>;
  }
  if (info.isEmpty) {
    return <p className="validation-error">This folder is empty.</p>;
  }

  return (
    <div className="project-summary">
      <h3>{info.name}</h3>
      <p className="project-summary__row">
        <span>Location:</span> {info.path}
      </p>
      <p className="project-summary__row">
        <span>Files:</span> {info.fileCount.toLocaleString()}
      </p>
      <p className="project-summary__row">
        <span>Size:</span> {formatSize(info.totalSizeBytes)}
      </p>
    </div>
  );
}
