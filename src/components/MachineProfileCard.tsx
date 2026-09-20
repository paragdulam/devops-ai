import { MACHINE_PROFILES } from "../data/machineProfiles";

// Rendered as a list even though there's only one profile today — cheap now,
// avoids a rewrite if a second profile shows up. No selection UI: the PRD
// says the machine configuration is hardcoded for the MVP.
export function MachineProfileCard() {
  return (
    <section className="machine-profile">
      <h2>Machine</h2>
      {MACHINE_PROFILES.map((profile) => (
        <div key={profile.id} className="machine-profile__card">
          <p>{profile.cpu} CPU</p>
          <p>{profile.ramGb} GB RAM</p>
        </div>
      ))}
    </section>
  );
}
