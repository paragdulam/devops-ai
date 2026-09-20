import { describe, expect, it } from "vitest";
import { createInitialState, rentalReducer } from "./rentalReducer";
import { RentalStatus, type Rental } from "../types/rental";
import type { ProjectInfo } from "../types/project";

const projectInfo: ProjectInfo = {
  name: "my-app",
  path: "/tmp/my-app",
  exists: true,
  readable: true,
  isEmpty: false,
  fileCount: 10,
  totalSizeBytes: 2048,
};

const rental: Rental = {
  id: "rental-1",
  status: RentalStatus.REQUESTED,
  ec2InstanceId: null,
  machineProfile: "standard",
  projectName: "my-app",
  createdAt: new Date().toISOString(),
  startedAt: null,
  stoppedAt: null,
  connection: null,
};

describe("rentalReducer", () => {
  it("stores the selected project", () => {
    const state = rentalReducer(createInitialState("standard"), {
      type: "PROJECT_SELECTED",
      projectInfo,
    });
    expect(state.projectInfo).toEqual(projectInfo);
  });

  it("stores a newly created rental", () => {
    const state = rentalReducer(createInitialState("standard"), {
      type: "RENTAL_CREATED",
      rental,
    });
    expect(state.rental).toEqual(rental);
  });

  it("applies rental updates", () => {
    const withRental = rentalReducer(createInitialState("standard"), {
      type: "RENTAL_CREATED",
      rental,
    });
    const updated: Rental = { ...rental, status: RentalStatus.RUNNING };
    const state = rentalReducer(withRental, { type: "RENTAL_UPDATED", rental: updated });
    expect(state.rental?.status).toBe(RentalStatus.RUNNING);
  });

  it("clears the rental back to null on RESET", () => {
    const withRental = rentalReducer(createInitialState("standard"), {
      type: "RENTAL_CREATED",
      rental,
    });
    const state = rentalReducer(withRental, { type: "RESET" });
    expect(state.rental).toBeNull();
  });
});
