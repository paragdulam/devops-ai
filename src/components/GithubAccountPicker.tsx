import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  deleteGithubAccount,
  getGithubLinkStatus,
  linkGithubAccount,
  listGithubAccounts,
} from "../lib/github";
import type { GithubAccount, GithubLinkStart } from "../types/github";

const POLL_INTERVAL_MS = 2000;

export function GithubAccountPicker({
  selectedAccountId,
  onSelect,
}: {
  selectedAccountId: string | null;
  onSelect: (accountId: string | null) => void;
}) {
  const [accounts, setAccounts] = useState<GithubAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [linking, setLinking] = useState<GithubLinkStart | null>(null);
  const [linkError, setLinkError] = useState<string | null>(null);
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelledRef = useRef(false);

  function refresh() {
    return listGithubAccounts().then((list) => {
      setAccounts(list);
      return list;
    });
  }

  useEffect(() => {
    refresh()
      .then((list) => {
        if (!selectedAccountId && list.length > 0) onSelect(list[0].id);
      })
      .finally(() => setLoading(false));
    return () => {
      cancelledRef.current = true;
      if (pollTimer.current) clearTimeout(pollTimer.current);
    };
    // Only fetch once on mount; re-linking/selecting is a user action, not
    // something this effect should react to.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleLink() {
    setLinkError(null);
    try {
      const start = await linkGithubAccount();
      setLinking(start);
      await openUrl(start.verificationUri);
      pollLinkStatus(start.linkId);
    } catch (err) {
      setLinkError(err instanceof Error ? err.message : String(err));
    }
  }

  function pollLinkStatus(linkId: string) {
    pollTimer.current = setTimeout(async () => {
      if (cancelledRef.current) return;
      try {
        const status = await getGithubLinkStatus(linkId);
        if (status.state === "linked") {
          setLinking(null);
          await refresh();
          onSelect(status.account.id);
          return;
        }
        if (status.state === "failed") {
          setLinking(null);
          setLinkError(status.error);
          return;
        }
        pollLinkStatus(linkId);
      } catch (err) {
        setLinking(null);
        setLinkError(err instanceof Error ? err.message : String(err));
      }
    }, POLL_INTERVAL_MS);
  }

  async function handleUnlink(id: string) {
    await deleteGithubAccount(id);
    const list = await refresh();
    if (selectedAccountId === id) {
      onSelect(list.length > 0 ? list[0].id : null);
    }
  }

  if (loading) return null;

  return (
    <div className="github-account-picker">
      {linking ? (
        <p className="github-account-picker__linking">
          Approve code <strong>{linking.userCode}</strong> at{" "}
          <button type="button" onClick={() => openUrl(linking.verificationUri)}>
            {linking.verificationUri}
          </button>
        </p>
      ) : accounts.length === 0 ? (
        <button type="button" onClick={handleLink}>
          Link GitHub
        </button>
      ) : (
        <div className="github-account-picker__row">
          <select
            value={selectedAccountId ?? accounts[0].id}
            onChange={(e) => onSelect(e.target.value)}
          >
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.login}
              </option>
            ))}
          </select>
          <button type="button" onClick={() => handleUnlink(selectedAccountId ?? accounts[0].id)}>
            Unlink
          </button>
        </div>
      )}
      {linkError && <p className="validation-error">{linkError}</p>}
    </div>
  );
}
