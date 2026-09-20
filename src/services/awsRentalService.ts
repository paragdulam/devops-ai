import { invoke } from "@tauri-apps/api/core";
import type { CreateRentalRequest, Rental } from "../types/rental";
import type { RentalService } from "./rentalService";

const POLL_INTERVAL_MS = 1500;

// Real EC2 provisioning, driven entirely by Rust Tauri commands
// (start_rental/get_rental/stop_rental) — see src-tauri/src/rentals/mod.rs.
// subscribeToRental polls get_rental rather than using Tauri events, mirroring
// the mock's timer-based style so no event plumbing is needed for this PoC.
export class AwsRentalService implements RentalService {
  createRental(req: CreateRentalRequest): Promise<Rental> {
    return invoke<Rental>("start_rental", {
      accountId: req.accountId,
      machineProfile: req.machineProfile,
      projectName: req.projectName,
      vmUsername: req.vmUsername,
      vmPassword: req.vmPassword,
      githubRepo: req.githubRepo,
    });
  }

  getRental(id: string): Promise<Rental> {
    return invoke<Rental>("get_rental", { id });
  }

  stopRental(id: string): Promise<Rental> {
    return invoke<Rental>("stop_rental", { id });
  }

  subscribeToRental(id: string, onUpdate: (rental: Rental) => void): () => void {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const poll = async () => {
      if (cancelled) return;
      try {
        const rental = await this.getRental(id);
        if (!cancelled) onUpdate(rental);
      } catch {
        // Transient IPC errors are swallowed; the next poll tries again.
      } 
      if (!cancelled) {
        timer = setTimeout(poll, POLL_INTERVAL_MS);
      }
    };

    void poll();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }
}
