import { Sidebar } from "../components/Sidebar";
import { useRentalState } from "../state/RentalContext";
import { rentalForKey } from "../state/rentalReducer";
import { useProjectDetection } from "../state/useProjectDetection";
import { useRentalSync } from "../state/useRentalSync";
import { RentalStatus } from "../types/rental";
import { RepoSetup } from "./RepoSetup";
import { VmDetail } from "./VmDetail";

// Master/detail landing screen: repos and rentals on the left, the selected
// one on the right. Every live rental keeps its own mounted VmDetail.
export function WorkspaceScreen({ onOpenAccounts }: { onOpenAccounts: () => void }) {
  const { state } = useRentalState();
  useProjectDetection();
  useRentalSync();

  const selectedRental = rentalForKey(state.rentals, state.selectedRepoKey);
  const showSetup =
    !selectedRental &&
    state.selectedGithubRepo !== null &&
    state.selectedRepoKey !== null;

  return (
    <div className="workspace">
      <Sidebar onOpenAccounts={onOpenAccounts} />
      <main className="workspace__detail">
        {Object.values(state.rentals)
          .filter((r) => r.status !== RentalStatus.RELEASED)
          .map((rental) => (
            <VmDetail key={rental.id} rental={rental} visible={rental.id === selectedRental?.id} />
          ))}
        {showSetup && <RepoSetup onOpenAccounts={onOpenAccounts} />}
        {!selectedRental && !showSetup && (
          <p className="workspace__empty">Select a repository or rental to get started.</p>
        )}
      </main>
    </div>
  );
}
