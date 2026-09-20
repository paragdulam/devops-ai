import { createContext, useContext, useMemo, useReducer, type ReactNode } from "react";
import { createRentalService, type RentalService } from "../services";
import { MACHINE_PROFILES } from "../data/machineProfiles";
import {
  createInitialState,
  rentalReducer,
  type AppAction,
  type AppState,
} from "./rentalReducer";

interface RentalContextValue {
  state: AppState;
  dispatch: React.Dispatch<AppAction>;
  rentalService: RentalService;
}

const RentalContext = createContext<RentalContextValue | null>(null);

export function RentalProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(
    rentalReducer,
    createInitialState(MACHINE_PROFILES[0].id),
  );
  const rentalService = useMemo(() => createRentalService(), []);

  const value = useMemo(
    () => ({ state, dispatch, rentalService }),
    [state, rentalService],
  );

  return <RentalContext.Provider value={value}>{children}</RentalContext.Provider>;
}

export function useRentalState(): RentalContextValue {
  const ctx = useContext(RentalContext);
  if (!ctx) {
    throw new Error("useRentalState must be used within a RentalProvider");
  }
  return ctx;
}
