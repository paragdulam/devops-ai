import { useEffect, useState } from "react";
import { useRentalState } from "../state/RentalContext";
import type { InstanceTypeOption } from "../types/rental";

function formatMemory(gib: number): string {
  return Number.isInteger(gib) ? `${gib}` : gib.toFixed(1);
}

function formatPrice(option: InstanceTypeOption): string {
  if (option.hourlyUsd == null) return "price unknown";
  const approx = option.priceSource === "fallback" ? "~" : "";
  return `${approx}$${option.hourlyUsd.toFixed(3)}/hr`;
}

// Lists the instance types the selected account's region offers that can run
// the Android emulator (backend: `list_instance_types`), cheapest first, and
// preselects the cheapest.
export function MachineProfileCard() {
  const { state, dispatch, rentalService } = useRentalState();
  const accountId = state.selectedAccountId;
  const [options, setOptions] = useState<InstanceTypeOption[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!accountId) {
      setOptions([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    rentalService
      .listInstanceTypes(accountId)
      .then((list) => {
        if (cancelled) return;
        setOptions(list);
        const current = state.selectedMachineProfileId;
        if (!list.some((o) => o.instanceType === current)) {
          dispatch({ type: "MACHINE_PROFILE_SELECTED", machineProfileId: list[0]?.instanceType ?? null });
        }
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setOptions([]);
        setError(err instanceof Error ? err.message : String(err));
        dispatch({ type: "MACHINE_PROFILE_SELECTED", machineProfileId: null });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // Refetch only when the account changes; the selection is handled above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accountId, rentalService]);

  return (
    <section className="machine-profile">
      <h2>Machine</h2>
      {!accountId ? (
        <p className="machine-profile__hint">Select an account to see available machines.</p>
      ) : loading ? (
        <p className="machine-profile__hint">Loading machines that can run the Android emulator…</p>
      ) : error ? (
        <p className="validation-error">Could not load machines: {error}</p>
      ) : (
        <ul className="machine-profile__list" role="radiogroup" aria-label="Machine">
          {options.map((option) => {
            const selected = option.instanceType === state.selectedMachineProfileId;
            return (
              <li key={option.instanceType}>
                <label
                  className={`machine-profile__option${selected ? " machine-profile__option--selected" : ""}`}
                >
                  <input
                    type="radio"
                    name="machine-profile"
                    value={option.instanceType}
                    checked={selected}
                    onChange={() =>
                      dispatch({
                        type: "MACHINE_PROFILE_SELECTED",
                        machineProfileId: option.instanceType,
                      })
                    }
                  />
                  <span className="machine-profile__type">{option.instanceType}</span>
                  <span>{option.vcpus} CPU</span>
                  <span>{formatMemory(option.memoryGib)} GB RAM</span>
                  <span className="machine-profile__price">{formatPrice(option)}</span>
                </label>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
