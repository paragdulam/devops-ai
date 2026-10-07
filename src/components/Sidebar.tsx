import { useEffect, useMemo, useState } from "react";
import {
  deleteGithubAccount,
  listGithubAccounts,
  listGithubRepos,
} from "../lib/github";
import { matchesQuery } from "../lib/repoFilter";
import {
  isLive,
  rentalForKey,
  rentalRepoKey,
  repoKey,
  type SidebarTab,
} from "../state/rentalReducer";
import { useRentalState } from "../state/RentalContext";
import { RentalStatus, type Rental } from "../types/rental";
import type { GithubAccount, GithubRepo } from "../types/github";
import { LinkGithubButton } from "./LinkGithubButton";
import { RentalCostEstimate } from "./RentalCost";

const TABS: { id: SidebarTab; label: string }[] = [
  { id: "repos", label: "Repositories" },
  { id: "rentals", label: "Rentals" },
];

function statusBadge(rental: Rental): { label: string; tone: string } {
  if (isLive(rental)) return { label: "● Running", tone: "live" };
  if (rental.status === RentalStatus.FAILED) return { label: "Failed", tone: "failed" };
  if (rental.status === RentalStatus.STOPPING) return { label: "Stopping…", tone: "busy" };
  return { label: "Starting…", tone: "busy" };
}

export function Sidebar({ onOpenAccounts }: { onOpenAccounts: () => void }) {
  const { state, dispatch } = useRentalState();
  const { sidebarTab, githubAccountId, selectedRepoKey, rentals } = state;

  const [accounts, setAccounts] = useState<GithubAccount[]>([]);
  const [accountsLoaded, setAccountsLoaded] = useState(false);
  const [repos, setRepos] = useState<GithubRepo[]>([]);
  const [reposLoading, setReposLoading] = useState(false);
  const [reposError, setReposError] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  async function refreshAccounts(): Promise<GithubAccount[]> {
    const list = await listGithubAccounts();
    setAccounts(list);
    return list;
  }

  useEffect(() => {
    refreshAccounts()
      .then((list) => {
        if (!githubAccountId && list.length > 0) {
          dispatch({ type: "GITHUB_ACCOUNT_SELECTED", accountId: list[0].id });
        }
      })
      .catch(() => {})
      .finally(() => setAccountsLoaded(true));
    // Only fetch once on mount; linking/unlinking refreshes explicitly.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!githubAccountId) {
      setRepos([]);
      return;
    }
    let cancelled = false;
    setReposLoading(true);
    setReposError(null);
    listGithubRepos(githubAccountId)
      .then((list) => {
        if (!cancelled) setRepos(list);
      })
      .catch((err) => {
        if (!cancelled) setReposError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setReposLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [githubAccountId]);

  async function handleLinked(account: GithubAccount) {
    await refreshAccounts();
    dispatch({ type: "GITHUB_ACCOUNT_SELECTED", accountId: account.id });
  }

  async function handleUnlink() {
    if (!githubAccountId) return;
    await deleteGithubAccount(githubAccountId);
    const list = await refreshAccounts();
    dispatch({ type: "GITHUB_ACCOUNT_SELECTED", accountId: list[0]?.id ?? null });
  }

  const visibleRepos = useMemo(
    () => repos.filter((r) => matchesQuery(query, r.fullName)),
    [repos, query],
  );

  // A repo "moves" to Rentals once its rental is live; it stays in Repositories too.
  const visibleRentals = useMemo(
    () =>
      Object.values(rentals)
        .filter(
          (r) =>
            isLive(r) &&
            r.githubRepo &&
            (!githubAccountId || r.githubRepo.githubAccountId === githubAccountId) &&
            matchesQuery(query, r.githubRepo.fullName, r.projectName),
        )
        .sort((a, b) => a.createdAt.localeCompare(b.createdAt)),
    [rentals, githubAccountId, query],
  );

  return (
    <aside className="sidebar">
      <div className="segmented" role="tablist" aria-label="Sidebar view">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={sidebarTab === tab.id}
            className={sidebarTab === tab.id ? "segmented__option active" : "segmented__option"}
            onClick={() => dispatch({ type: "SIDEBAR_TAB_CHANGED", tab: tab.id })}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {accounts.length > 0 && (
        <div className="sidebar__account">
          <select
            aria-label="GitHub account"
            value={githubAccountId ?? ""}
            onChange={(e) => dispatch({ type: "GITHUB_ACCOUNT_SELECTED", accountId: e.target.value })}
          >
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.login}
              </option>
            ))}
          </select>
          <button type="button" onClick={handleUnlink}>
            Unlink
          </button>
        </div>
      )}

      <input
        type="search"
        className="sidebar__search"
        placeholder={sidebarTab === "repos" ? "Search repositories…" : "Search rentals…"}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />

      <ul className="sidebar__list">
        {sidebarTab === "repos" ? (
          <>
            {visibleRepos.map((repo) => {
              const key = repoKey(githubAccountId!, repo.fullName);
              const rental = rentalForKey(rentals, key);
              const badge = rental ? statusBadge(rental) : null;
              return (
                <li key={repo.id}>
                  <button
                    type="button"
                    className={`sidebar__item${key === selectedRepoKey ? " selected" : ""}`}
                    onClick={() =>
                      dispatch({
                        type: "GITHUB_REPO_SELECTED",
                        selection: { accountId: githubAccountId!, repo },
                      })
                    }
                  >
                    <span className="sidebar__item-name">
                      {repo.fullName}
                      {repo.private ? " 🔒" : ""}
                    </span>
                    {badge && (
                      <span className={`sidebar__badge sidebar__badge--${badge.tone}`}>
                        {badge.label}
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
            {accountsLoaded && !githubAccountId && (
              <li className="sidebar__empty">Link a GitHub account to see your repositories.</li>
            )}
            {reposLoading && <li className="sidebar__empty">Loading repositories…</li>}
            {reposError && <li className="validation-error">{reposError}</li>}
            {githubAccountId && !reposLoading && !reposError && visibleRepos.length === 0 && (
              <li className="sidebar__empty">
                {query ? "No repositories match." : "No repositories found."}
              </li>
            )}
          </>
        ) : (
          <>
            {visibleRentals.map((rental) => {
              const key = rentalRepoKey(rental);
              return (
                <li key={rental.id}>
                  <button
                    type="button"
                    className={`sidebar__item${key === selectedRepoKey ? " selected" : ""}`}
                    onClick={() => dispatch({ type: "RENTAL_SELECTED", rentalId: rental.id })}
                  >
                    <span className="sidebar__item-name">{rental.githubRepo!.fullName}</span>
                    <span className="sidebar__badge sidebar__badge--live">
                      ● Running <RentalCostEstimate rental={rental} compact />
                    </span>
                  </button>
                </li>
              );
            })}
            {visibleRentals.length === 0 && (
              <li className="sidebar__empty">
                {query ? "No rentals match." : "No rentals yet. Start one from a repository."}
              </li>
            )}
          </>
        )}
      </ul>

      <div className="sidebar__footer">
        <button type="button" className="sidebar__manage-accounts" onClick={onOpenAccounts}>
          Manage Accounts
        </button>
        <LinkGithubButton onLinked={handleLinked} />
      </div>
    </aside>
  );
}
