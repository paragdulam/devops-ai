import { useState } from "react";
import { ProjectPicker } from "../components/ProjectPicker";
import { MachineProfileCard } from "../components/MachineProfileCard";
import { IdePicker } from "../components/IdePicker";
import { AccountPicker } from "../components/AccountPicker";
import { VmCredentialsForm } from "../components/VmCredentialsForm";
import { useRentalState } from "../state/RentalContext";
import { isProjectValid } from "../types/project";
import type { CreateRentalRequest } from "../types/rental";

export function HomeScreen({ onOpenAccounts }: { onOpenAccounts: () => void }) {
  const { state, dispatch, rentalService } = useRentalState();
  const [starting, setStarting] = useState(false);

  const hasValidLocalProject =
    state.projectSource === "local" && state.projectInfo !== null && isProjectValid(state.projectInfo);
  const hasValidGithubRepo = state.projectSource === "github" && state.selectedGithubRepo !== null;

  const hasVmCredentials = state.vmUsername.trim() !== "" && state.vmPassword !== "";

  const canStart =
    (hasValidLocalProject || hasValidGithubRepo) &&
    state.selectedAccountId !== null &&
    hasVmCredentials &&
    !starting;

  async function handleStartRenting() {
    if (!state.selectedAccountId || (!hasValidLocalProject && !hasValidGithubRepo)) return;

    const request: CreateRentalRequest =
      state.projectSource === "github" && state.selectedGithubRepo
        ? {
            accountId: state.selectedAccountId,
            machineProfile: state.selectedMachineProfileId,
            projectName: state.selectedGithubRepo.repo.name,
            vmUsername: state.vmUsername,
            vmPassword: state.vmPassword,
            githubRepo: {
              githubAccountId: state.selectedGithubRepo.accountId,
              repoName: state.selectedGithubRepo.repo.name,
              fullName: state.selectedGithubRepo.repo.fullName,
              cloneUrl: state.selectedGithubRepo.repo.cloneUrl,
              defaultBranch: state.selectedGithubRepo.repo.defaultBranch,
            },
            ides: state.selectedIdes,
          }
        : {
            accountId: state.selectedAccountId,
            machineProfile: state.selectedMachineProfileId,
            projectName: state.projectInfo!.name,
            vmUsername: state.vmUsername,
            vmPassword: state.vmPassword,
            ides: state.selectedIdes,
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
    <main className="home-screen">
      <h1>Remote Dev</h1>
      <ProjectPicker />
      <IdePicker />
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
    </main>
  );
}
