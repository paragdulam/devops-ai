import { useEffect, useState, type FormEvent } from "react";
import { addCloudAccount, deleteCloudAccount, listCloudAccounts } from "../lib/accounts";
import type { CloudAccount } from "../types/account";
import { ConfirmDialog } from "../components/ConfirmDialog";

function maskAccessKey(key: string): string {
  if (key.length <= 8) return key;
  return `${key.slice(0, 4)}…${key.slice(-4)}`;
}

export function AccountsScreen({ onBack }: { onBack: () => void }) {
  const [accounts, setAccounts] = useState<CloudAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [label, setLabel] = useState("");
  const [region, setRegion] = useState("ap-south-1");
  const [accessKeyId, setAccessKeyId] = useState("");
  const [secretAccessKey, setSecretAccessKey] = useState("");
  const [adding, setAdding] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);

  function refresh() {
    return listCloudAccounts().then(setAccounts);
  }

  useEffect(() => {
    refresh().finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleAdd(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    setAdding(true);
    try {
      await addCloudAccount({
        label,
        provider: "aws",
        region,
        accessKeyId,
        secretAccessKey,
      });
      setLabel("");
      setAccessKeyId("");
      setSecretAccessKey("");
      await refresh();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setAdding(false);
    }
  }

  async function handleDelete(id: string) {
    setPendingDeleteId(null);
    await deleteCloudAccount(id);
    await refresh();
  }

  return (
    <main className="accounts-screen">
      <h1>Accounts</h1>

      {loading ? (
        <p>Loading…</p>
      ) : accounts.length === 0 ? (
        <p className="accounts-screen__empty">No saved accounts yet.</p>
      ) : (
        <ul className="accounts-screen__list">
          {accounts.map((account) => (
            <li key={account.id} className="accounts-screen__item">
              <div>
                <strong>{account.label}</strong>
                <p className="accounts-screen__item-meta">
                  {account.provider.toUpperCase()} · {account.region} ·{" "}
                  {maskAccessKey(account.accessKeyId)}
                </p>
              </div>
              <button type="button" onClick={() => setPendingDeleteId(account.id)}>
                Delete
              </button>
            </li>
          ))}
        </ul>
      )}

      <form className="accounts-screen__form" onSubmit={handleAdd}>
        <h2>Add AWS Account</h2>
        <label>
          Label
          <input value={label} onChange={(e) => setLabel(e.target.value)} required />
        </label>
        <label>
          Region
          <input value={region} onChange={(e) => setRegion(e.target.value)} required />
        </label>
        <label>
          Access Key ID
          <input value={accessKeyId} onChange={(e) => setAccessKeyId(e.target.value)} required />
        </label>
        <label>
          Secret Access Key
          <input
            type="password"
            value={secretAccessKey}
            onChange={(e) => setSecretAccessKey(e.target.value)}
            required
          />
        </label>
        {formError && <p className="validation-error">{formError}</p>}
        <button type="submit" disabled={adding}>
          {adding ? "Validating…" : "Add Account"}
        </button>
      </form>

      <button type="button" className="accounts-screen__back" onClick={onBack}>
        Back
      </button>

      {pendingDeleteId && (
        <ConfirmDialog
          message="Remove this saved account? Its credentials will be deleted from this device's keychain."
          confirmLabel="Remove"
          onConfirm={() => handleDelete(pendingDeleteId)}
          onCancel={() => setPendingDeleteId(null)}
        />
      )}
    </main>
  );
}
