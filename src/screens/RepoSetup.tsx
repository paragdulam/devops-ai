import { useState } from "react";
import { MachineProfileCard } from "../components/MachineProfileCard";
import { IdePicker } from "../components/IdePicker";
import { ToolchainPicker } from "../components/ToolchainPicker";
import { AccountPicker } from "../components/AccountPicker";
import { VmCredentialsForm } from "../components/VmCredentialsForm";
import { useRentalState } from "../state/RentalContext";
import { isValidToolVersion } from "../types/project";
import type { CreateRentalRequest } from "../types/rental";

// Detail pane for a repo with no rental yet: configure and start one.
export function RepoSetup({ onOpenAccounts }: { onOpenAccounts: () => void }) {
  const { state, dispatch, rentalService } = useRentalState();
  const [starting, setStarting] = useState(false);
  const selection = state.selectedGithubRepo;
  if (!selection) return null;

  const hasVmCredentials = state.vmUsername.trim() !== "" && state.vmPassword !== "";
  const hasValidTools = state.selectedTools.every((t) => isValidToolVersion(t.version));
  const canStart =
    hasValidTools && state.selectedAccountId !== null && hasVmCredentials && !starting;

  async function handleStartRenting() {
    if (!selection || !state.selectedAccountId) return;

    const request: CreateRentalRequest = {
      accountId: state.selectedAccountId,
      machineProfile: state.selectedMachineProfileId,
      projectName: selection.repo.name,
      vmUsername: state.vmUsername,
      vmPassword: state.vmPassword,
      githubRepo: {
        githubAccountId: selection.accountId,
        repoName: selection.repo.name,
        fullName: selection.repo.fullName,
        cloneUrl: selection.repo.cloneUrl,
        defaultBranch: selection.repo.defaultBranch,
      },
      ides: state.selectedIdes,
      tools: state.selectedTools,
    };

    setStarting(true);
    try {
      const rental = await rentalService.createRental(request);
      dispatch({ type: "RENTAL_CREATED", rental });
    } catch (err) {
      dispatch({
        type: "RENTAL_ERROR",
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setStarting(false);
    }
  }

  return (
    <section className="repo-setup">
      <h1>{selection.repo.fullName}</h1>
      <IdePicker />
      <ToolchainPicker />
      <MachineProfileCard />
      <AccountPicker onManageAccounts={onOpenAccounts} />
      <VmCredentialsForm />
      {state.error && <p className="validation-error">{state.error}</p>}
      <button
        type="button"
        className="start-renting-button"
        disabled={!canStart}
        onClick={handleStartRenting}
      >
        START RENTING
      </button>
    </section>
  );
}
