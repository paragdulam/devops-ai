# Domain Model

The core domain types exist twice — once in Rust (`src-tauri/src/`), once in TypeScript (`src/types/`) — and are kept in sync by convention (matching field names/shapes), not by shared codegen. Rust uses `snake_case` internally but serializes with `#[serde(rename_all = "camelCase")]`, so the JSON crossing the Tauri IPC boundary always matches the TypeScript interfaces below field-for-field.

## Class diagram

```mermaid
classDiagram
    class CloudAccount_Rust["CloudAccount (Rust)"] {
        +String id
        +String label
        +String provider
        +String region
        +String access_key_id
        +String created_at
    }

    class CloudAccount_TS["CloudAccount (TS)"] {
        +string id
        +string label
        +CloudProvider provider
        +string region
        +string accessKeyId
        +string createdAt
    }

    class AddCloudAccountRequest_TS["AddCloudAccountRequest (TS)"] {
        +string label
        +CloudProvider provider
        +string region
        +string accessKeyId
        +string secretAccessKey
    }

    class ProjectInfo_Rust["ProjectInfo (Rust)"] {
        +String name
        +String path
        +bool exists
        +bool readable
        +bool is_empty
        +u64 file_count
        +u64 total_size_bytes
    }

    class ProjectInfo_TS["ProjectInfo (TS)"] {
        +string name
        +string path
        +boolean exists
        +boolean readable
        +boolean isEmpty
        +number fileCount
        +number totalSizeBytes
        +isProjectValid() boolean
    }

    class RentalRecord_Rust["RentalRecord (Rust, server-side only)"] {
        +String id
        +RentalStatus status
        +String account_id
        +String machine_profile
        +String project_name
        +Option~String~ ec2_instance_id
        +Option~String~ security_group_id
        +Option~String~ public_ip
        +Option~String~ vnc_password
        +String created_at
        +Option~String~ started_at
        +Option~String~ stopped_at
        +Option~String~ error
        +to_dto() RentalDto
    }

    class RentalDto_Rust["RentalDto (Rust, crosses IPC)"] {
        +String id
        +RentalStatus status
        +Option~String~ ec2_instance_id
        +String machine_profile
        +String project_name
        +String created_at
        +Option~String~ started_at
        +Option~String~ stopped_at
        +Option~String~ error
        +Option~RentalConnection~ connection
    }

    class Rental_TS["Rental (TS)"] {
        +string id
        +RentalStatus status
        +string|null ec2InstanceId
        +string machineProfile
        +string projectName
        +string createdAt
        +string|null startedAt
        +string|null stoppedAt
        +string? error
        +RentalConnection|null connection
    }

    class RentalConnection_Rust["RentalConnection (Rust)"] {
        +String public_ip
        +u16 vnc_port
        +String vnc_password
    }

    class RentalConnection_TS["RentalConnection (TS)"] {
        +string publicIp
        +number vncPort
        +string vncPassword
    }

    class CreateRentalRequest_TS["CreateRentalRequest (TS)"] {
        +string accountId
        +string machineProfile
        +string projectName
    }

    class RentalStatus_enum["RentalStatus «enum»"] {
        REQUESTED
        PROVISIONING
        BOOTING
        CONNECTING
        READY
        RUNNING
        STOPPING
        RELEASED
        FAILED
    }

    class MachineProfile_TS["MachineProfile (TS)"] {
        +string id
        +string label
        +number cpu
        +number ramGb
        +string provider
    }

    RentalRecord_Rust --> RentalDto_Rust : to_dto()
    RentalRecord_Rust --> RentalStatus_enum
    RentalDto_Rust --> RentalStatus_enum
    RentalDto_Rust --> RentalConnection_Rust : connection
    Rental_TS --> RentalStatus_enum
    Rental_TS --> RentalConnection_TS : connection
    RentalDto_Rust ..> Rental_TS : "IPC JSON\n(camelCase, 1:1)"
    RentalConnection_Rust ..> RentalConnection_TS : "IPC JSON"
    CloudAccount_Rust ..> CloudAccount_TS : "IPC JSON"
    ProjectInfo_Rust ..> ProjectInfo_TS : "IPC JSON"
```

## Notes on the mapping

- **`RentalRecord` vs `RentalDto`/`Rental`** is the one intentional asymmetry: `RentalRecord` is the server-side source of truth held in the in-memory `HashMap<String, RentalRecord>` (`src-tauri/src/rentals/mod.rs`) and includes `account_id`, `security_group_id`, and `vnc_password` in a form used internally. `RentalDto` (and its TS mirror `Rental`) is what actually crosses IPC — `account_id` and `security_group_id` are **dropped**, and `vnc_password` only appears nested inside `connection`, which itself is only populated once both `public_ip` and `vnc_password` are set (i.e. once the instance is running and the password has been generated).
- **`CloudAccount` never carries the secret.** The AWS secret access key is written to the OS keyring at account-creation time and is never part of the `CloudAccount` struct/interface that flows back to the frontend or gets persisted to `accounts.json`.
- **`RentalStatus` is a shared enum in name only** — it's defined independently in Rust (`rentals/mod.rs`, serialized `UPPERCASE`) and TypeScript (`types/rental.ts`), with identical string values (`"REQUESTED"`, `"PROVISIONING"`, ...). See [`RENTAL_LIFECYCLE.md`](RENTAL_LIFECYCLE.md) for the full state machine.
- **`MachineProfile`** exists only in TypeScript (`src/data/machineProfiles.ts`) as a hardcoded single entry (`id: "standard"`, 8 CPU / 32 GB RAM). Its `id` is passed as the `machine_profile` string to `start_rental`, which the Rust side resolves via `instance_type_for_profile("standard") -> "m5.2xlarge"` (`aws/instance.rs`) — there's no shared `MachineProfile` struct on the Rust side, just a string match.
- **`ProjectInfo`** is produced entirely by Rust (`inspect_project_folder`, in `commands/project.rs`) by walking the selected directory and skipping `IGNORE_DIRS` (`.git`, `node_modules`, `target`, `dist`, `build`, `.venv`, `__pycache__`, `.next`, `.turbo`). The frontend never computes these fields itself.

## Related docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — where these types sit in the overall system
- [`RENTAL_LIFECYCLE.md`](RENTAL_LIFECYCLE.md) — how `RentalStatus` transitions over time
- [`IPC_API_REFERENCE.md`](IPC_API_REFERENCE.md) — exact command signatures that produce/consume these types
