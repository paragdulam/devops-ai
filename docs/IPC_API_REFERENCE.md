# IPC API Reference

The frontend and Rust backend communicate exclusively through **Tauri commands** — `@tauri-apps/api/core`'s `invoke(command, args)` calling into `#[tauri::command]` functions registered in `src-tauri/src/lib.rs`'s `tauri::generate_handler![...]`. Argument objects use camelCase keys; Tauri/serde maps them onto the Rust functions' snake_case parameters automatically.

There are exactly **7 registered commands** — this is the entire IPC surface of the app.

## Commands

### `inspect_project_folder`

| | |
|---|---|
| **Rust fn** | `commands/project.rs` |
| **Params** | `path: String` |
| **Returns** | `Result<ProjectInfo, String>` |
| **Frontend call site** | `src/lib/project.ts: inspectProjectFolder(path)` |
| **Used by** | `ProjectPicker.tsx`, after the user picks a folder via the native dialog |

Walks the directory off the async runtime (`spawn_blocking`), skipping `IGNORE_DIRS` (`.git`, `node_modules`, `target`, `dist`, `build`, `.venv`, `__pycache__`, `.next`, `.turbo`), and returns file count / total size / existence / readability / emptiness.

---

### `list_cloud_accounts`

| | |
|---|---|
| **Rust fn** | `commands/accounts.rs` |
| **Params** | *(none — `app: AppHandle` is injected automatically)* |
| **Returns** | `Result<Vec<CloudAccount>, String>` |
| **Frontend call site** | `src/lib/accounts.ts: listCloudAccounts()` |
| **Used by** | `AccountPicker.tsx`, `AccountsScreen.tsx` |

Reads `accounts.json` from the app's local data directory. Returns an empty list if the file doesn't exist yet.

---

### `add_cloud_account`

| | |
|---|---|
| **Rust fn** | `commands/accounts.rs` |
| **Params** | `label: String, provider: String, region: String, access_key_id: String, secret_access_key: String` |
| **Returns** | `Result<CloudAccount, String>` |
| **Frontend call site** | `src/lib/accounts.ts: addCloudAccount(req)` |
| **Used by** | `AccountsScreen.tsx` add-account form |

Validates the credentials **live** against AWS via `sts::get_caller_identity()` before saving anything. On success: generates a UUID, stores the secret in the OS keyring (service `com.paragdulam.remotedevmachine.accounts`, entry key = account id), appends the account (without the secret) to `accounts.json`. Only `provider: "aws"` is accepted today.

---

### `delete_cloud_account`

| | |
|---|---|
| **Rust fn** | `commands/accounts.rs` |
| **Params** | `id: String` |
| **Returns** | `Result<(), String>` |
| **Frontend call site** | `src/lib/accounts.ts: deleteCloudAccount(id)` |
| **Used by** | `AccountsScreen.tsx` delete flow (behind `ConfirmDialog`) |

Deletes the keyring entry (best-effort) and removes the account from `accounts.json`.

---

### `start_rental`

| | |
|---|---|
| **Rust fn** | `commands/rentals.rs` |
| **Params** | `account_id: String, machine_profile: String, project_name: String` |
| **Returns** | `Result<RentalDto, String>` |
| **Frontend call site** | `src/services/awsRentalService.ts: AwsRentalService.createRental(req)` |
| **Used by** | `HomeScreen.tsx` "START RENTING" button |

Creates a `RentalRecord` (status `REQUESTED`), inserts it into the in-memory map, and returns its DTO **immediately** — then spawns `rentals::provision(...)` as a detached background task that does the actual AWS work. See [`RENTAL_LIFECYCLE.md`](RENTAL_LIFECYCLE.md) for the full provisioning sequence.

---

### `get_rental`

| | |
|---|---|
| **Rust fn** | `commands/rentals.rs` |
| **Params** | `id: String` |
| **Returns** | `Result<RentalDto, String>` |
| **Frontend call site** | `src/services/awsRentalService.ts: AwsRentalService.getRental(id)` |
| **Used by** | `AwsRentalService.subscribeToRental`'s 1.5-second poll loop (there is no push/event mechanism — the frontend polls this command repeatedly while `RentalScreen` is mounted) |

Pure read from the in-memory map; errors if no rental with that id exists.

---

### `stop_rental`

| | |
|---|---|
| **Rust fn** | `commands/rentals.rs` |
| **Params** | `id: String` |
| **Returns** | `Result<RentalDto, String>` |
| **Frontend call site** | `src/services/awsRentalService.ts: AwsRentalService.stopRental(id)` |
| **Used by** | `StopRentingButton.tsx` (behind `ConfirmDialog`) |

If the rental is already `RELEASED`, returns immediately with no side effects. Otherwise flips status to `STOPPING` synchronously and returns that DTO, then spawns `rentals::stop(...)` as a detached background task to terminate the instance and release the security group.

## Non-`invoke()` bridges

- **Native folder picker**: `@tauri-apps/plugin-dialog`'s `open({ directory: true })`, used by `pickProjectFolder()` in `src/lib/project.ts`. Backed by the `tauri_plugin_dialog` plugin registered in `lib.rs`, permissioned via `dialog:allow-open` in `src-tauri/capabilities/default.json`. Not a custom `#[tauri::command]` — it's the plugin's own API.

## What is NOT IPC

`RemoteDesktopPanel.tsx` connects **directly from the browser webview to the EC2 instance's public IP** over a raw WebSocket (`ws://<publicIp>:6080/`) using `@novnc/novnc`'s `RFB` client, once the rental's `connection` field is populated. This bypasses Rust entirely — it is not a Tauri command and does not appear in `generate_handler!`.

## Related docs

- [`DOMAIN_MODEL.md`](DOMAIN_MODEL.md) — the `CloudAccount`/`ProjectInfo`/`RentalDto` types these commands exchange
- [`RENTAL_LIFECYCLE.md`](RENTAL_LIFECYCLE.md) — sequence diagrams showing these commands in context
