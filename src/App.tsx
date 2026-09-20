import { useState } from "react";
import { RentalProvider, useRentalState } from "./state/RentalContext";
import { HomeScreen } from "./screens/HomeScreen";
import { RentalScreen } from "./screens/RentalScreen";
import { AccountsScreen } from "./screens/AccountsScreen";
import "./App.css";

type View = "home" | "accounts";

function Router() {
  const { state } = useRentalState();
  const [view, setView] = useState<View>("home");

  if (state.rental) return <RentalScreen />;
  if (view === "accounts") return <AccountsScreen onBack={() => setView("home")} />;
  return <HomeScreen onOpenAccounts={() => setView("accounts")} />;
}

function App() {
  return (
    <RentalProvider>
      <Router />
    </RentalProvider>
  );
}

export default App;
