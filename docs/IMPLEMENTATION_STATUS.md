# Implementation Status

[`PRD.md`](PRD.md) describes the intended MVP, including its 7 development milestones (§26) and a 3-tier architecture with a separate Go backend + Go `rental-agent` (§18, §25). This doc tracks what the *current code* actually does against that plan, so new developers don't mistake the PRD's aspirational design for the real system. See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the as-built architecture.

## Milestone status

| PRD Milestone | Status | Notes |
|---|---|---|
| **1 — Local UI** (project picker, machine selection, Start Renting button, rental screen) | ✅ Done | `HomeScreen`, `ProjectPicker`, `MachineProfileCard`, `RentalScreen` all exist and work. |
| **2 — AWS provisioning** (Start Renting → backend → EC2 API → Ubuntu VM) | ✅ Done — but **not** via a separate backend | `rentals::provision` in `src-tauri/src/rentals/mod.rs` does this directly from Rust, not from a Go service. Functionally equivalent outcome, different architecture. |
| **3 — Remote agent** (VM → agent → backend, "machine is ready") | ⚠️ Partial / different approach | No `rental-agent` process runs on the VM. Readiness is inferred by the Rust side: a retrying SSH auth-handshake probe (`aws::instance::wait_for_ssh_ready`) gates the start of Ansible provisioning, and a raw TCP probe of port 6080 (`wait_for_port_open`) gates the final `Ready` transition — no in-VM agent reports in either way. |
| **4 — Terminal** (xterm.js → WebSocket → Agent → PTY → bash) | ✅ Done — different transport | `TerminalPanel.tsx` is a real `@xterm/xterm` terminal, opened on demand (not always mounted). Instead of a WebSocket to a remote agent, it's backed by a direct SSH PTY session from the Rust layer (`src-tauri/src/ssh/terminal.rs`, using `russh`), streamed to the frontend over Tauri events. |
| **5 — Graphical desktop** (XFCE + x11vnc + noVNC embedded in Tauri) | ✅ Done — with a substituted VNC server | XFCE + noVNC work as specified; the VNC server is **TigerVNC (`vncserver`)**, not x11vnc. `RemoteDesktopPanel.tsx` fully embeds it via `@novnc/novnc`. |
| **6 — Local repository** (choose folder → archive → upload → remote VM → `/workspace/project`) | ⚠️ Partial | `inspect_project_folder` (folder validation + stats) is done. Archiving, uploading, and placing the project at `/workspace/<name>` on the remote machine are **not implemented** — a rental currently provisions a bare dev machine with no project on it. The ignore-list used for folder inspection (`.git`, `node_modules`, etc.) is explicitly reused-in-waiting for this future archiver. |
| **7 — Complete rental loop** (project → start → VM → project upload → desktop → terminal → run app → stop → VM terminated) | ⚠️ Partial | Everything works end-to-end **except** project upload and the terminal — a developer can rent a machine, see its graphical desktop, and release it, but cannot yet get their code onto it or run commands against it from within the app. |

## Architectural deviations from the PRD

| PRD says (§18, §25) | Actual |
|---|---|
| Separate **Go backend** service exposing `POST /rentals`, `GET /rentals/:id`, `POST /rentals/:id/stop`, `WS /rentals/:id/terminal`, `WS /rentals/:id/agent` | No backend service exists. All of this logic runs **in-process inside the Tauri app's Rust layer**, invoked via Tauri IPC commands instead of HTTP/WebSocket. See [`IPC_API_REFERENCE.md`](IPC_API_REFERENCE.md). |
| **Go `rental-agent`** running on the VM, connecting outbound to the backend | Does not exist. The instance's readiness is inferred externally (TCP probe on the noVNC port), not reported by an in-VM process. |
| **Packer**-built VM image | The VM is stock Ubuntu 22.04 (resolved via SSM public parameter), with no custom AMI built ahead of time. Configuration is no longer cloud-init either — it's done post-boot over SSH by a generated **Ansible playbook** (`src-tauri/src/ansible/mod.rs`), using an app-generated, per-rental EC2 keypair. This is a bigger departure from the PRD than the original cloud-init approach was: neither Packer nor cloud-init nor Ansible appear in the PRD, which only ever describes a Go `rental-agent`. |
| **x11vnc** as the VNC server | **TigerVNC (`vncserver`)** is used instead; behavior is equivalent for this use case. |
| PostgreSQL "not required until persistence becomes necessary" | Still true — rentals remain a pure in-memory `HashMap`. Accounts, unlike the PRD's silence on the topic, **do** persist (JSON file + OS keyring), since account management wasn't explicitly scoped by the PRD but was needed to actually drive AWS. |

## Other known limitations (found during codebase review, not in the PRD)

- **No rental persistence/recovery.** If the app is closed or crashes while a rental is `RUNNING`, the in-memory record is lost — the EC2 instance and its security group keep running/costing money with no record in the app to stop them. There's no reconciliation against AWS on next launch.
- **Single default VPC/subnet assumption.** `resolve_default_vpc_and_subnet` (`aws/network.rs`) requires exactly one default VPC with at least one subnet in the target account/region and errors otherwise — documented in-code as a PoC limitation.
- **Mock service not runtime-selectable.** `MockRentalService` is fully implemented and unit-tested but `services/index.ts`'s `createRentalService()` always returns `AwsRentalService` — there's no way to run the UI against the mock outside of tests (e.g. for demos without AWS credentials).
- **Single machine profile, but now provisioning-aware.** Only `"standard"` (`m5.2xlarge`, 8 CPU / 32 GB RAM as labeled in the UI) exists in `aws::instance::PROFILES`; anything else errors. Profiles now carry an `ansible_packages` list (empty today) so a future profile can drive different Ansible package installs, not just a different instance type — but there's still no catalog UI, and no second profile exists yet.
- **Ansible must be preinstalled by the developer.** The app shells out to `ansible-playbook` on `PATH` (`ansible::preflight`) rather than bundling Python/Ansible itself. **Windows is explicitly unsupported** for this reason — Ansible has no native Windows control-node support, and `provision()` fails fast with that message on `cfg!(target_os = "windows")` rather than surfacing a confusing "ansible-playbook not found" error.
- **Project toolchain via mise, best-effort.** Tools detected from the README (first) and version files are installed with `mise use --global` in the Ansible `toolchain` phase, and the cloned repo gets a `mise install`. Each install is `ignore_errors`, so a bad README guess or unavailable version is logged and doesn't fail the rental. The README parser is rule-based, so free-form prose it doesn't recognize is missed (the user can add tools in `ToolchainPicker`). mise itself is installed from `https://mise.run` at its latest release and is not version-pinned yet. Local-folder projects aren't uploaded to the VM (Milestone 6), so for those only the global installs run.
- **`StrictHostKeyChecking` is disabled** for both the Ansible connection and the interactive terminal (`ssh::connect`'s `AcceptAllHandler` accepts any host key). This is a deliberate, scoped trust-on-first-use relaxation — every connection target is an instance this app itself just created moments earlier via a freshly imported keypair, with no prior host key to check against — not an oversight.
- **`RentalStatus` duplicated by convention.** The Rust and TypeScript enums are hand-kept in sync (same string values) with no shared schema/codegen — a change to one without the other would silently break IPC parsing. See [`DOMAIN_MODEL.md`](DOMAIN_MODEL.md).

## What's solid

- **Security posture is take-it-seriously good for a PoC, with one deliberate change**: the security group is scoped to the caller's own `/32` IP on **both** port 6080 (noVNC) and port 22 (SSH) — SSH is no longer closed, since the app's own Ansible provisioning and the on-demand terminal both need it. It is still never widened beyond the caller's own IP. VNC stays bound `localhost`-only on the instance with websockify as the sole externally reachable noVNC hop; AWS credentials remain always explicit from the app's own vault (never the default SDK chain); account secrets, the per-rental SSH private key, and GitHub tokens are all OS-keyring-only (never in a JSON file, never sent back over IPC).
- **Cleanup is symmetric and best-effort everywhere.** The same `cleanup()` function (terminate instance → wait → delete security group → delete EC2 keypair → delete keyring entry → delete generated Ansible artifacts) backs both the explicit Stop Renting path and every provisioning-failure path (`cleanup_and_fail`), so a failed rental doesn't leave orphaned AWS resources, keys, or files running/lingering.
- **The `RentalService` seam is real**, even though only one implementation is wired up at runtime — swapping in a different transport (HTTP, if a backend is ever added) only requires a new `RentalService` implementation, not UI changes.

## Related docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — the as-built system architecture
- [`RENTAL_LIFECYCLE.md`](RENTAL_LIFECYCLE.md) — exact state transitions and timings referenced above
- [`FRONTEND_GUIDE.md`](FRONTEND_GUIDE.md) — `TerminalPanel` and the mock-service wiring in more detail
