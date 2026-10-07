import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getGithubLinkStatus, linkGithubAccount } from "../lib/github";
import type { GithubAccount, GithubLinkStart } from "../types/github";

const POLL_INTERVAL_MS = 2000;

// GitHub device flow: show the code, send the user to the verification page,
// poll until they approve.
export function LinkGithubButton({ onLinked }: { onLinked: (account: GithubAccount) => void }) {
  const [linking, setLinking] = useState<GithubLinkStart | null>(null);
  const [linkError, setLinkError] = useState<string | null>(null);
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelledRef = useRef(false);

  useEffect(() => {
    cancelledRef.current = false;
    return () => {
      cancelledRef.current = true;
      if (pollTimer.current) clearTimeout(pollTimer.current);
    };
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
        if (cancelledRef.current) return;
        if (status.state === "linked") {
          setLinking(null);
          onLinked(status.account);
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

  return (
    <div className="link-github">
      {linking ? (
        <p className="link-github__linking">
          Approve code <strong>{linking.userCode}</strong> at{" "}
          <button type="button" onClick={() => openUrl(linking.verificationUri)}>
            {linking.verificationUri}
          </button>
        </p>
      ) : (
        <button type="button" className="link-github__button" onClick={handleLink}>
          Link GitHub
        </button>
      )}
      {linkError && <p className="validation-error">{linkError}</p>}
    </div>
  );
}
