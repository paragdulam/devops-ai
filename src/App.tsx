import { useState } from "react";
import { RentalProvider } from "./state/RentalContext";
import { WorkspaceScreen } from "./screens/WorkspaceScreen";
import { AccountsScreen } from "./screens/AccountsScreen";
import "./App.css";

type View = "home" | "accounts";

function Router() {
  const [view, setView] = useState<View>("home");

  // The workspace stays mounted (just hidden) behind the accounts screen so
  // open VNC and terminal sessions survive a trip to manage accounts.
  return (
    <>
      <div className="app-view" hidden={view !== "home"}>
        <WorkspaceScreen onOpenAccounts={() => setView("accounts")} />
      </div>
      {view === "accounts" && <AccountsScreen onBack={() => setView("home")} />}
    </>
  );
}

function App() {
  return (
    <RentalProvider>
      <Router />
    </RentalProvider>
  );
}

export default App;
