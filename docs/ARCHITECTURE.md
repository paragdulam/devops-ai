# Architecture

## System summary

Remote Dev Machine is a **single Tauri desktop application** — a React/TypeScript UI running inside a Rust host process — that lets a developer rent a temporary AWS EC2 machine, work on it through an embedded graphical desktop (noVNC), and release it when done. There is **no separate backend service**: all AWS orchestration and the rental state machine run in-process inside the Rust layer (`src-tauri`), as background Tokio tasks spawned by Tauri IPC commands.

This is a deliberate divergence from [`PRD.md`](PRD.md), which describes a 3-tier design with a standalone Go backend and a Go `rental-agent` running on the VM. See [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) for the full breakdown of what's built vs. still aspirational.

## Tech stack

| Layer                | PRD says                         | Actually used                                                                                                                                  |
| -------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop shell        | Tauri                            | Tauri 2 ✅                                                                                                                                     |
| Frontend             | React + TypeScript               | React 19 + TypeScript ✅                                                                                                                       |
| Native/backend logic | Go backend (separate service)    | **Rust, inside `src-tauri`** — no Go anywhere in this repo                                                                            |
| Cloud                | AWS EC2 via AWS SDK for Go v2    | AWS EC2/SSM/STS via**`aws-sdk-ec2`/`aws-sdk-ssm`/`aws-sdk-sts` (Rust)**, called directly from Tauri commands                       |
| Remote OS            | Ubuntu + XFCE, built via Packer  | Ubuntu 22.04 (resolved live via SSM public parameter) + XFCE, configured post-boot over SSH by a generated **Ansible playbook** (`src-tauri/src/ansible`) — no Packer image, no cloud-init either |
| Remote display       | x11vnc + noVNC                   | **TigerVNC (`vncserver`) + `python3-websockify` + noVNC** ✅ (different VNC server, same shape; now installed/enabled as systemd units by Ansible instead of a cloud-init `runcmd`) |
| Remote terminal      | xterm.js + Go remote-agent + PTY | **`@xterm/xterm`, opened on demand, over a direct SSH PTY session** (`src-tauri/src/ssh/terminal.rs`, using `russh`) instead of a WebSocket to a Go remote-agent |
| Rental storage       | In-memory (MVP)                  | In-memory`HashMap` inside the Tauri process ✅ (matches MVP scope)                                                                           |
| Account storage      | — (not specified)               | JSON file (`accounts.json` in app-local-data-dir) + secret key in OS keyring                                                                 |

## Component diagram

```mermaid
graph TB
    subgraph TauriApp["Tauri Desktop Application (single process)"]
        subgraph Frontend["React / TypeScript UI (webview)"]
            Screens["Screens & Components<br/>(Home, Accounts, Rental)"]
            State["RentalContext / rentalReducer"]
            Services["RentalService<br/>(AwsRentalService)"]
        end

        subgraph RustBackend["Rust native layer (src-tauri)"]
            Commands["Tauri Commands<br/>(accounts.rs, project.rs, rentals.rs)"]
            RentalsEngine["Rentals Engine<br/>(rentals/mod.rs)<br/>in-memory state machine"]
            AwsModules["AWS Modules<br/>(aws/credentials, ami, network,<br/>instance, keypair)"]
            AnsibleEngine["Ansible Engine<br/>(ansible/mod.rs)<br/>generates inventory/playbook,<br/>shells out to ansible-playbook"]
            SshEngine["SSH<br/>(ssh/mod.rs, ssh/terminal.rs)<br/>russh client — readiness probe,<br/>key materialization, terminal PTY"]
            AccountStore["Account Store<br/>accounts.json + OS keyring"]
        end
    end

    subgraph AWS["AWS (customer account)"]
        STS["STS"]
        SSM["SSM Parameter Store"]
        EC2API["EC2 API"]
        subgraph Instance["Provisioned EC2 Instance"]
            Sshd["sshd<br/>(key-only, from imported keypair)"]
            XFCE["XFCE Desktop"]
            Xvnc["Xvnc (localhost-only)"]
            Websockify["websockify :6080"]
        end
    end

    Screens --> State
    State --> Services
    Services -- "Tauri invoke()" --> Commands
    Commands --> RentalsEngine
    Commands --> AccountStore
    RentalsEngine --> AwsModules
    RentalsEngine --> AnsibleEngine
    RentalsEngine --> SshEngine
    AwsModules --> STS
    AwsModules --> SSM
    AwsModules --> EC2API
    EC2API -. "launches" .-> Instance
    AnsibleEngine -- "SSH (provisioning)" --> Sshd
    SshEngine -- "SSH (readiness probe,<br/>on-demand terminal)" --> Sshd
    Sshd --> XFCE --> Xvnc --> Websockify

    Frontend -- "noVNC WebSocket<br/>(direct, bypasses Rust)" --> Websockify
```

Note the noVNC connection: once a rental reaches `READY`, `RemoteDesktopPanel.tsx` opens a WebSocket **directly from the webview to the EC2 instance's public IP** on port 6080 — it does not proxy through Rust.

## Deployment diagram

```mermaid
graph LR
    subgraph DevMachine["Developer's Local Machine"]
        App["Remote Dev Machine.app<br/>(Tauri process)"]
        Keyring["OS Keyring<br/>(AWS secret keys)"]
        LocalFS["Local filesystem<br/>accounts.json, selected project folder"]
        App --- Keyring
        App --- LocalFS
    end

    subgraph AWSCloud["AWS Cloud (customer's account/region)"]
        subgraph VPC["Default VPC"]
            SG["Security Group rdm-<rental-id><br/>TCP 6080 AND TCP 22, each open<br/>ONLY to caller's /32 IP"]
            subgraph EC2["EC2 instance (m5.2xlarge)"]
                direction TB
                OS["Ubuntu 22.04<br/>(imported keypair authorizes<br/>the 'ubuntu' user's SSH key)"]
                Desktop["XFCE + Xvnc :1 (localhost-only)<br/>provisioned by Ansible over SSH"]
                WS["websockify 0.0.0.0:6080 → localhost:5901"]
                OS --> Desktop --> WS
            end
            SG -.-> EC2
        end
    end

    App -- "AWS SDK calls<br/>(EC2 / SSM / STS)" --> AWSCloud
    App -- "noVNC over WebSocket<br/>ws://<public-ip>:6080/" --> WS
    App -- "SSH (Ansible provisioning,<br/>readiness probe, on-demand terminal)" --> OS
```

Security posture baked into this diagram:

- The security group opens **port 6080 and port 22**, **only to the caller's own current public `/32` IP** — never `0.0.0.0/0`. SSH is now deliberately open (a reversal of the earlier cloud-init-era design), since the app's own Ansible provisioning and on-demand terminal both need it; it is still never widened beyond the caller's own IP.
- VNC itself (`Xvnc`) binds `localhost`-only on the instance; websockify is the only externally reachable noVNC hop.
- SSH access is **keypair-only** — the app generates a fresh Ed25519 keypair per rental, imports it as the EC2 key pair, and never enables password authentication. The private key lives only in the OS keyring, materialized to a `0600` file just before each use and deleted immediately after.
- AWS credentials are always the explicit key pair from the app's own account vault — the default SDK credential chain (env vars, `~/.aws/*`) is never consulted.

## Why no Go backend?

The PRD's 3-tier design (Tauri app ↔ Go backend ↔ Go `rental-agent`) was the intended end-state, but the current implementation took a shortcut for the MVP: fold the backend responsibilities (Rental API, AWS EC2 management, machine state, security) directly into the Rust layer that ships inside the Tauri app. This works because there's exactly one desktop client and no need for a shared/multi-user backend yet. See [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) for the milestone-by-milestone comparison against the PRD.


## Related docs

- [`DOMAIN_MODEL.md`](DOMAIN_MODEL.md) — core types and how Rust/TypeScript mirror each other
- [`RENTAL_LIFECYCLE.md`](RENTAL_LIFECYCLE.md) — state machine and sequence diagrams for start/stop/failure
- [`IPC_API_REFERENCE.md`](IPC_API_REFERENCE.md) — the Tauri command contract between frontend and Rust
- [`FRONTEND_GUIDE.md`](FRONTEND_GUIDE.md) — screens/components/state layout
