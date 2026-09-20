import { RentalStatus, type CreateRentalRequest, type Rental } from "../types/rental";
import type { RentalService } from "./rentalService";

// How long each step of the fake provisioning progression takes. Tunable so
// demos/tests can speed the whole "boot" up.
export const STEP_DELAY_MS = 400;

// The states a rental walks through after REQUESTED, in order, ending at RUNNING.
const PROGRESSION: RentalStatus[] = [
  RentalStatus.PROVISIONING,
  RentalStatus.BOOTING,
  RentalStatus.CONNECTING,
  RentalStatus.READY,
  RentalStatus.RUNNING,
];

type Listener = (rental: Rental) => void;

// In-memory storage, mirroring the PRD §19 note that the real backend also
// starts out in-memory-only for the MVP — same storage strategy either way.
export class MockRentalService implements RentalService {
  private rentals = new Map<string, Rental>();
  private listeners = new Map<string, Set<Listener>>();

  private notify(id: string, rental: Rental) {
    this.listeners.get(id)?.forEach((cb) => cb(rental));
  }

  private update(id: string, patch: Partial<Rental>) {
    const current = this.rentals.get(id);
    if (!current) return;
    const updated = { ...current, ...patch };
    this.rentals.set(id, updated);
    this.notify(id, updated);
  }

  private scheduleProgression(id: string) {
    let step = 0;
    const advance = () => {
      if (step >= PROGRESSION.length) return;
      const status = PROGRESSION[step];
      const patch: Partial<Rental> = { status };
      if (status === RentalStatus.PROVISIONING) {
        patch.ec2InstanceId = `i-mock${id.replace(/-/g, "").slice(0, 12)}`;
      }
      if (status === RentalStatus.RUNNING) {
        patch.startedAt = new Date().toISOString();
      }
      this.update(id, patch);
      step += 1;
      if (step < PROGRESSION.length) {
        setTimeout(advance, STEP_DELAY_MS);
      }
    };
    setTimeout(advance, STEP_DELAY_MS);
  }

  createRental(req: CreateRentalRequest): Promise<Rental> {
    const id = crypto.randomUUID();
    const rental: Rental = {
      id,
      status: RentalStatus.REQUESTED,
      ec2InstanceId: null,
      machineProfile: req.machineProfile,
      projectName: req.projectName,
      createdAt: new Date().toISOString(),
      startedAt: null,
      stoppedAt: null,
      connection: null,
    };
    this.rentals.set(id, rental);
    this.scheduleProgression(id);
    return Promise.resolve(rental);
  }

  getRental(id: string): Promise<Rental> {
    const rental = this.rentals.get(id);
    if (!rental) return Promise.reject(new Error(`Rental ${id} not found`));
    return Promise.resolve(rental);
  }

  stopRental(id: string): Promise<Rental> {
    const rental = this.rentals.get(id);
    if (!rental) return Promise.reject(new Error(`Rental ${id} not found`));

    this.update(id, { status: RentalStatus.STOPPING });
    setTimeout(() => {
      this.update(id, {
        status: RentalStatus.RELEASED,
        stoppedAt: new Date().toISOString(),
      });
    }, STEP_DELAY_MS);

    return Promise.resolve({ ...rental, status: RentalStatus.STOPPING });
  }

  subscribeToRental(id: string, onUpdate: Listener): () => void {
    if (!this.listeners.has(id)) this.listeners.set(id, new Set());
    this.listeners.get(id)!.add(onUpdate);
    return () => {
      this.listeners.get(id)?.delete(onUpdate);
    };
  }
}
