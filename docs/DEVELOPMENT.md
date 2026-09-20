# Development Guide

## Prerequisites

- **Node.js** + **pnpm** (the project uses `pnpm-lock.yaml`; other package managers aren't tested)
- **Rust toolchain** (stable, edition 2021) — install via [rustup](https://rustup.rs/)
- **Tauri CLI system dependencies** for your OS (WebView runtime, build tools) — see the [Tauri prerequisites guide](https://tauri.app/start/prerequisites/)
- **AWS account** with EC2/SSM/STS permissions if you want to actually run a rental end-to-end (the app validates and stores your own AWS access key/secret at runtime — no shared credentials are baked into the repo)

## Install

```bash
pnpm install
```

## Running

| Command | What it does |
|---|---|
| `pnpm tauri dev` | Runs the full desktop app (Rust + webview) in dev mode with hot reload. This is what you want for normal development. |
| `pnpm dev` | Runs only the Vite dev server (`http://localhost:1420`) for the React frontend, without the Tauri/Rust shell. Useful for pure UI work, but Tauri `invoke()` calls will fail since there's no Rust host. |
| `pnpm build` | Type-checks (`tsc`) then builds the frontend (`vite build`) — the frontend half of a production build. |
| `pnpm tauri build` | Full production build (frontend + Rust binary + platform installer). |
| `pnpm test` | Runs the Vitest suite (`vitest run`) — frontend unit tests only. Rust tests run separately via `cargo test` inside `src-tauri/`. |

`src-tauri/tauri.conf.json` wires `beforeDevCommand: pnpm dev` and `beforeBuildCommand: pnpm build`, so `pnpm tauri dev`/`pnpm tauri build` invoke those automatically — you don't need to run them separately.

## Where things are stored locally

| What | Where |
|---|---|
| Saved AWS accounts (no secrets) | `accounts.json` in the app's local data directory (`app_local_data_dir()`, OS-specific) |
| AWS secret access keys | OS keyring, service name `com.paragdulam.remotedevmachine.accounts`, entry key = account UUID |
| Active rentals | **In memory only**, inside the running Tauri process — lost on app restart, with no reconciliation of orphaned EC2 instances on relaunch |

If you need to reset local state during development, delete `accounts.json` from the app-local-data directory and/or remove the matching keyring entries; there's no CLI for this yet.

## Project layout

```text
DevOpsAI/
├── docs/                        Onboarding docs (this set) + PRD.md
├── src/                          React/TypeScript frontend
│   ├── screens/                   Home, Accounts, Rental — see FRONTEND_GUIDE.md
│   ├── components/                 UI building blocks
│   ├── state/                       RentalContext + rentalReducer (app state machine)
│   ├── services/                     RentalService interface + Aws/Mock implementations
│   ├── lib/                           Tauri invoke() wrappers + small pure helpers
│   ├── types/                          Domain types mirroring the Rust structs
│   ├── data/                            Hardcoded machine profile catalog
│   └── App.tsx                          Top-level provider + router
└── src-tauri/                    Rust native layer
    └── src/
        ├── main.rs / lib.rs        Entry point, plugin/state registration, command handler
        ├── commands/                 Tauri #[tauri::command] functions (the IPC contract)
        │   ├── accounts.rs
        │   ├── project.rs
        │   └── rentals.rs
        ├── aws/                       AWS SDK integration, one module per concern
        │   ├── credentials.rs
        │   ├── ami.rs
        │   ├── network.rs
        │   ├── instance.rs
        │   └── userdata.rs
        └── rentals/mod.rs             The rental state machine + in-memory store
```

## Adding a new Tauri command

Follow the existing pattern (e.g. `inspect_project_folder`):

1. Write the function in the relevant `src-tauri/src/commands/*.rs` file (or a new module), annotated `#[tauri::command]`, returning `Result<T, String>` where `T: Serialize`.
2. Register it in `src-tauri/src/lib.rs`'s `tauri::generate_handler![...]` list.
3. Add a thin wrapper in `src/lib/*.ts` (or wherever fits) calling `invoke<T>("command_name", { ...args })` — match argument names in camelCase; Tauri maps them to the Rust function's snake_case params automatically.
4. If the command returns a new shape, add/extend the matching TypeScript interface in `src/types/` — see [`DOMAIN_MODEL.md`](DOMAIN_MODEL.md) for the existing Rust ↔ TS mirroring convention.

Full command reference: [`IPC_API_REFERENCE.md`](IPC_API_REFERENCE.md).

## Testing

- **Frontend**: `pnpm test` (Vitest). Existing suites: `lib/rentalChecklist.test.ts`, `lib/time.test.ts`, `services/mockRentalService.test.ts`, `state/rentalReducer.test.ts`.
- **Rust**: `cd src-tauri && cargo test`. Existing suite: `commands/project.rs`'s `#[cfg(test)] mod tests` (directory-walking edge cases).
- There is no end-to-end test that actually provisions an EC2 instance — verifying the full rental flow currently means running `pnpm tauri dev` against a real AWS account.

## Related docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system overview and diagrams
- [`FRONTEND_GUIDE.md`](FRONTEND_GUIDE.md) — screens/components/state/services map
- [`IPC_API_REFERENCE.md`](IPC_API_REFERENCE.md) — full Tauri command reference
- [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) — what's built vs. still planned
