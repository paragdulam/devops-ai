import { useEffect, useState } from "react";
import { listCloudAccounts } from "../lib/accounts";
import type { CloudAccount } from "../types/account";
import { useRentalState } from "../state/RentalContext";

export function AccountPicker({ onManageAccounts }: { onManageAccounts: () => void }) {
  const { state, dispatch } = useRentalState();
  const [accounts, setAccounts] = useState<CloudAccount[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    listCloudAccounts()
      .then((list) => {
        if (cancelled) return;
        setAccounts(list);
        if (!state.selectedAccountId && list.length > 0) {
          dispatch({ type: "ACCOUNT_SELECTED", accountId: list[0].id });
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // Only fetch once on mount; re-selecting an account is a user action, not
    // something this effect should react to.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (loading) return null;

  return (
    <section className="account-picker">
      <h2>Account</h2>
      {accounts.length === 0 ? (
        <p className="account-picker__empty">
          No AWS accounts yet.{" "}
          <button type="button" onClick={onManageAccounts}>
            Add one
          </button>
        </p>
      ) : (
        <div className="account-picker__row">
          <select
            value={state.selectedAccountId ?? ""}
            onChange={(e) => dispatch({ type: "ACCOUNT_SELECTED", accountId: e.target.value })}
          >
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.label} ({account.region})
              </option>
            ))}
          </select>
          <button type="button" onClick={onManageAccounts}>
            Manage Accounts
          </button>
        </div>
      )}
    </section>
  );
}
