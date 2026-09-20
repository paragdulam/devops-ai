import { useRentalState } from "../state/RentalContext";

// Sets the Linux OS account (login + sudo password) Ansible creates on the
// VM — not SSH auth, which is always keypair-only. Shown at rental start so
// the account exists before provisioning begins.
export function VmCredentialsForm() {
  const { state, dispatch } = useRentalState();

  function update(vmUsername: string, vmPassword: string) {
    dispatch({ type: "VM_CREDENTIALS_CHANGED", vmUsername, vmPassword });
  }

  return (
    <div className="vm-credentials-form">
      <label>
        VM username
        <input
          type="text"
          value={state.vmUsername}
          onChange={(e) => update(e.target.value, state.vmPassword)}
          placeholder="e.g. dev"
          autoComplete="off"
        />
      </label>
      <label>
        VM password
        <input
          type="password"
          value={state.vmPassword}
          onChange={(e) => update(state.vmUsername, e.target.value)}
          placeholder="Login + sudo password"
          autoComplete="new-password"
        />
      </label>
    </div>
  );
}
