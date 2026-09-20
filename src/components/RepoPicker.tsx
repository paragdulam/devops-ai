import { useEffect, useState } from "react";
import { listGithubRepos } from "../lib/github";
import type { GithubRepo } from "../types/github";
import { useRentalState } from "../state/RentalContext";

export function RepoPicker({ accountId }: { accountId: string | null }) {
  const { state, dispatch } = useRentalState();
  const [repos, setRepos] = useState<GithubRepo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!accountId) {
      setRepos([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    listGithubRepos(accountId)
      .then((list) => {
        if (!cancelled) setRepos(list);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [accountId]);

  if (!accountId) {
    return <p className="repo-picker__placeholder">Link a GitHub account to browse repos.</p>;
  }

  if (loading) return <p className="repo-picker__placeholder">Loading repos…</p>;

  return (
    <div className="repo-picker">
      {error && <p className="validation-error">{error}</p>}
      {repos.length === 0 && !error ? (
        <p className="repo-picker__placeholder">No repos found.</p>
      ) : (
        <select
          value={state.selectedGithubRepo?.repo.id ?? ""}
          onChange={(e) => {
            const repo = repos.find((r) => String(r.id) === e.target.value);
            if (!repo) return;
            dispatch({ type: "GITHUB_REPO_SELECTED", selection: { accountId, repo } });
          }}
        >
          <option value="" disabled>
            Select a repo…
          </option>
          {repos.map((repo) => (
            <option key={repo.id} value={repo.id}>
              {repo.fullName}
              {repo.private ? " (private)" : ""}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}
