# Remote Dev Machine

A Tauri desktop app that lets a developer rent a temporary AWS EC2 machine, work on it through an embedded graphical desktop, and release it when done.

**Select Project → Start Renting → Work → Stop Renting**

The goal: make a powerful remote Linux machine feel like a temporary extension of your own computer, without ever touching the AWS console, SSHing in, or configuring VNC yourself.

Built with Tauri 2 (Rust) + React 19 + TypeScript. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how it fits together — in short, this app does *not* yet match the separate-Go-backend design in the original PRD; all AWS orchestration currently runs in-process inside the Rust layer. See [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) for the full gap analysis.

## Quick start

```bash
pnpm install
pnpm tauri dev   # runs the full desktop app (Rust + webview)
pnpm test        # frontend unit tests (Vitest)
```

See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) for prerequisites, all available commands, and where local state (accounts, credentials) is stored.

## Documentation

| Doc                                                               | What's in it                                                                |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------- |
| [`docs/PRD.md`](docs/PRD.md)                                     | Product requirements — the original product goal, user flow, and MVP scope |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)                   | System overview, tech stack, component & deployment diagrams                |
| [`docs/DOMAIN_MODEL.md`](docs/DOMAIN_MODEL.md)                   | Core types (class diagram) and how Rust/TypeScript mirror each other        |
| [`docs/RENTAL_LIFECYCLE.md`](docs/RENTAL_LIFECYCLE.md)           | Rental state machine, plus sequence diagrams for start/stop/failure         |
| [`docs/IPC_API_REFERENCE.md`](docs/IPC_API_REFERENCE.md)         | Every Tauri command exchanged between frontend and Rust                     |
| [`docs/FRONTEND_GUIDE.md`](docs/FRONTEND_GUIDE.md)               | Screens, components, state management, and the service layer                |
| [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)                     | Setup, running, testing, project layout, how to add a new command           |
| [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) | What's built vs. still planned, milestone by milestone                      |

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
