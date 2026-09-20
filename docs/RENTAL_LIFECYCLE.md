# Rental Lifecycle

A rental moves through a fixed sequence of `RentalStatus` values, driven entirely by the Rust side (`src-tauri/src/rentals/mod.rs`) and mirrored to the frontend by 1.5-second polling of `get_rental` (`src/services/awsRentalService.ts`).

## State diagram

```mermaid
stateDiagram-v2
    [*] --> REQUESTED: start_rental() creates record,\nreturns immediately
    REQUESTED --> PROVISIONING: provision() task starts:\nresolve account + instance type
    PROVISIONING --> BOOTING: instance launched,\nreached Running + got public IP
    BOOTING --> CONNECTING: fixed 40s grace delay\n(cloud-init readiness proxy)
    CONNECTING --> READY: port 6080 reachable\n(websockify TCP probe)
    READY --> RUNNING: started_at recorded
    RUNNING --> STOPPING: user clicks STOP RENTING\n(stop_rental marks status\nsynchronously, returns immediately)
    STOPPING --> RELEASED: stop() background task:\nterminate instance, delete SG

    PROVISIONING --> FAILED: any AWS call errors\n(cleanup_and_fail)
    BOOTING --> FAILED: wait_for_running timeout/error
    CONNECTING --> FAILED: wait_for_port_open timeout
    RELEASED --> [*]
    FAILED --> [*]

    note right of RELEASED
        Frontend dispatches RESET on seeing
        RELEASED, routing back to HomeScreen.
    end note

    note right of FAILED
        cleanup_and_fail() best-effort
        terminates the instance and deletes
        the security group before marking FAILED.
    end note
```

## Sequence: Start Renting

```mermaid
sequenceDiagram
    actor Dev as Developer
    participant UI as HomeScreen
    participant Svc as AwsRentalService
    participant Cmd as rentals.rs (Tauri command)
    participant Eng as rentals::provision (background task)
    participant AWS as AWS (STS/SSM/EC2)
    participant VM as EC2 Instance

    Dev->>UI: click START RENTING
    UI->>Svc: createRental({accountId, machineProfile, projectName})
    Svc->>Cmd: invoke("start_rental", ...)
    Cmd->>Cmd: new_record() status=REQUESTED,\ninsert into in-memory map
    Cmd-->>Svc: RentalDto (REQUESTED)
    Svc-->>UI: Rental
    UI->>UI: dispatch RENTAL_CREATED → routes to RentalScreen

    Note over Cmd,Eng: start_rental spawns provision()\nas a detached background task

    Eng->>Eng: resolve_account_secret (keyring)
    Eng->>Eng: instance_type_for_profile("standard")
    Eng->>Eng: status → PROVISIONING
    Eng->>AWS: build_sdk_config + resolve_ubuntu_ami (SSM)
    Eng->>AWS: resolve_default_vpc_and_subnet
    Eng->>AWS: lookup_caller_ip (checkip.amazonaws.com)
    Eng->>AWS: create_scoped_security_group (port 6080, caller /32)
    Eng->>AWS: launch_instance (run_instances)
    AWS-->>VM: instance created, cloud-init runs
    Eng->>AWS: wait_for_running (poll describe_instances)
    AWS-->>Eng: Running + public IP
    Eng->>Eng: status → BOOTING
    Eng->>Eng: sleep 40s (cloud-init grace period)
    Eng->>Eng: status → CONNECTING
    Eng->>VM: wait_for_port_open(publicIp, 6080)
    VM-->>Eng: TCP connect succeeds
    Eng->>Eng: status → READY, started_at set
    Eng->>Eng: status → RUNNING

    loop every 1.5s (RentalScreen mounted)
        UI->>Svc: subscribeToRental poll → getRental(id)
        Svc->>Cmd: invoke("get_rental", {id})
        Cmd-->>Svc: RentalDto (current status)
        Svc-->>UI: dispatch RENTAL_UPDATED
    end

    UI->>VM: noVNC WebSocket ws://publicIp:6080/\n(direct, once READY/RUNNING)
```

## Sequence: Stop Renting

```mermaid
sequenceDiagram
    actor Dev as Developer
    participant UI as StopRentingButton
    participant Svc as AwsRentalService
    participant Cmd as rentals.rs (Tauri command)
    participant Eng as rentals::stop (background task)
    participant AWS as AWS (EC2)

    Dev->>UI: click STOP RENTING → confirm
    UI->>Svc: stopRental(id)
    Svc->>Cmd: invoke("stop_rental", {id})
    Cmd->>Cmd: status → STOPPING (synchronous)
    Cmd-->>Svc: RentalDto (STOPPING)

    Note over Cmd,Eng: stop_rental spawns stop()\nas a detached background task

    Eng->>Eng: resolve_account_secret
    Eng->>AWS: terminate_instance
    Eng->>AWS: wait_for_terminated (poll, budget 180s)
    Eng->>AWS: delete_security_group (retry up to 8x)
    Eng->>Eng: stopped_at set, status → RELEASED

    loop polling (RentalScreen still mounted)
        UI->>Svc: getRental(id)
        Svc->>Cmd: invoke("get_rental", {id})
        Cmd-->>Svc: RentalDto (RELEASED)
    end
    UI->>UI: dispatch RENTAL_UPDATED then RESET\n→ routes back to HomeScreen
```

## Sequence: Provisioning failure (cleanup path)

```mermaid
sequenceDiagram
    participant Eng as rentals::provision
    participant AWS as AWS (EC2)
    participant Rec as In-memory RentalRecord

    Note over Eng: Any step from launch_instance onward\ncan fail (timeout, AWS error, etc.)

    Eng->>AWS: e.g. wait_for_running times out
    Eng->>Eng: cleanup_and_fail(instance_id, security_group_id, error)
    Eng->>AWS: terminate_instance (best-effort)
    Eng->>AWS: wait_for_terminated (best-effort, 180s budget)
    Eng->>AWS: delete_security_group (best-effort, retries)
    Eng->>Rec: status → FAILED, error = message

    Note over Rec: Same cleanup() function is shared with\nthe explicit Stop path — nothing billed\nor exposed is ever left behind.
```

## Frontend checklist mapping

`src/lib/rentalChecklist.ts` maps each `RentalStatus` to a 5-step UI checklist (`Finding machine → Creating VM → Booting Ubuntu → Connecting to remote machine → Preparing workspace`), rendered by `RentalStateChecklist.tsx`. `RUNNING`/`STOPPING`/`RELEASED` show all steps done; `FAILED` flags the first step as errored (the UI can't know which real backend step failed, so it doesn't attempt to guess beyond that).

## Related docs

- [`DOMAIN_MODEL.md`](DOMAIN_MODEL.md) — `RentalStatus`, `RentalRecord`, `RentalDto`/`Rental` shapes
- [`IPC_API_REFERENCE.md`](IPC_API_REFERENCE.md) — `start_rental`/`get_rental`/`stop_rental` command signatures
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — where this state machine sits in the overall system
