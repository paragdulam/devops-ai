import type { ActualCost, CreateRentalRequest, Rental } from "../types/rental";

// The seam Milestone 2 fills in with a real HTTP/WebSocket client.
// Maps directly to the PRD §18 API surface:
//   createRental -> POST /rentals
//   getRental    -> GET  /rentals/:id
//   stopRental   -> POST /rentals/:id/stop
// subscribeToRental has no direct backend endpoint yet — in this milestone
// it's the mock's timer; later it becomes a poller against GET /rentals/:id
// and/or a listener on WS /rentals/:id/agent. Whatever the transport,
// callers above this interface never change.
export interface RentalService {
  createRental(req: CreateRentalRequest): Promise<Rental>;
  getRental(id: string): Promise<Rental>;
  // Rentals the backend already knows about (restored from disk at launch).
  listRentals(): Promise<Rental[]>;
  // Re-scopes the rental's firewall to this machine's current public IP.
  refreshAccess(id: string): Promise<void>;
  // Billed cost from AWS Cost Explorer (lags ~24h; each call costs $0.01).
  getActualCost(id: string, force?: boolean): Promise<ActualCost>;
  stopRental(id: string): Promise<Rental>;
  subscribeToRental(id: string, onUpdate: (rental: Rental) => void): () => void;
}
