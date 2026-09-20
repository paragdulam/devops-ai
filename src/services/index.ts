import type { RentalService } from "./rentalService";
import { AwsRentalService } from "./awsRentalService";

// The single integration point built in Milestone 1 for exactly this swap:
// real EC2 provisioning now lives in AwsRentalService (src-tauri/src/rentals),
// invoked over Tauri commands. Nothing else in the app needed to change.
export function createRentalService(): RentalService {
  return new AwsRentalService();
}

export type { RentalService } from "./rentalService";
