# Remote Development Machine — Basic MVP

**Status:** Draft  
**Version:** 0.1  
**Objective:** Validate the core **Start Renting → Work → Stop Renting** experience.

---

## 1. Product Goal

Build a desktop application that allows a developer to:

1. Select a local code repository.
2. Start renting a remote Linux machine.
3. Have the repository transferred to that machine.
4. Access the remote machine through a graphical desktop.
5. Open an integrated terminal.
6. Run and interact with the application remotely.
7. Stop renting and release the remote machine.

The MVP should make the remote machine feel like a temporary, more powerful computer attached to the developer's desktop.

### Core interaction

**Select Project → Start Renting → Work → Stop Renting**

---

## 2. What We Are NOT Building

The MVP deliberately excludes:

- GitHub integration
- GitHub authentication
- iOS
- Android emulator
- GPU support
- Multiple cloud providers
- Kubernetes
- Multi-machine environments
- sophisticated billing
- user accounts
- team collaboration
- persistent workspaces
- automatic scaling
- production-grade cloud orchestration

The purpose of the MVP is to validate the **remote computer experience**.

---

## 3. Target User

A developer who has a local application that would benefit from more powerful hardware.

Example:

```text
Local MacBook
8 CPU
16 GB RAM

Project:
large Node/Java/Python application

Problem:
Build/test environment is slow locally.
```

They can rent:

```text
Remote machine
16 CPU
64 GB RAM
```

and work on the application remotely.

---

## 4. MVP User Flow

```text
Open application
      ↓
Select local repository
      ↓
Select machine
      ↓
START RENTING
      ↓
Provision remote machine
      ↓
Transfer repository
      ↓
Connect to graphical desktop
      ↓
Open terminal
      ↓
Run application
      ↓
Developer works
      ↓
STOP RENTING
      ↓
Remote machine released
```

---

## 5. Desktop Application

### Technology

- Tauri
- React
- TypeScript
- Rust

The desktop application is the primary interface.

### 5.1 Home Screen

The user sees:

```text
┌───────────────────────────────────────┐
│ Remote Dev                            │
│                                       │
│ Project                               │
│                                       │
│ ~/Projects/my-app                     │
│                                       │
│             [ Choose Folder ]         │
│                                       │
│ Machine                               │
│                                       │
│ 8 CPU                                 │
│ 32 GB RAM                             │
│                                       │
│             [ START RENTING ]         │
└───────────────────────────────────────┘
```

For MVP, the machine configuration can be hard-coded.

Example:

> AWS EC2 — 8 CPU / 32 GB RAM

---

## 6. Local Repository Selection

The user selects a local directory through the native folder picker.

The application should verify that the selected directory is valid.

Minimum validation:

- directory exists
- directory is readable
- directory is not empty

The application should display:

```text
Project

my-app

Location:
~/Projects/my-app

Files:
1,842

Size:
327 MB
```

---

## 7. Start Renting

The primary action is:

> **START RENTING**

When clicked, the desktop application calls the backend.

```http
POST /rentals
```

Example request:

```json
{
  "machineProfile": "standard"
}
```

The backend creates the rental record and starts provisioning.

---

## 8. Rental States

The application must represent the rental lifecycle.

```text
REQUESTED
    ↓
PROVISIONING
    ↓
BOOTING
    ↓
CONNECTING
    ↓
READY
    ↓
RUNNING
    ↓
STOPPING
    ↓
RELEASED
```

Error state:

```text
FAILED
```

The UI should clearly communicate the current state.

Example:

```text
Starting your machine...

✓ Finding machine
✓ Creating VM
✓ Booting Ubuntu
● Connecting to remote machine
○ Preparing workspace
```

---

## 9. Cloud Infrastructure

For MVP, use **AWS EC2 only**.

No provider abstraction is required yet.

### AWS components

- EC2
- EBS
- Security Groups
- IAM
- S3 if needed for project transfer

### AWS SDK

Use:

**AWS SDK for Go v2**

The backend should be able to:

- create instance
- query instance state
- retrieve connection information
- stop/terminate instance
- terminate failed provisioning attempts

---

## 10. Remote Machine

The initial VM image should be a preconfigured Ubuntu image.

It should contain:

```text
Ubuntu
│
├── XFCE
├── x11vnc
├── Git
├── Docker
├── common development tools
└── rental-agent
```

The image should be created using **Packer**.

The VM should start without requiring manual configuration.

---

## 11. Graphical Desktop

The remote machine must provide a complete graphical Linux desktop.

Use:

### XFCE

as the desktop environment.

Example:

```text
┌─────────────────────────────────────────┐
│ Applications    Places       System     │
├─────────────────────────────────────────┤
│                                         │
│                                         │
│              Ubuntu XFCE                │
│                                         │
│                                         │
│                                         │
├─────────────────────────────────────────┤
│                                         │
│                                         │
└─────────────────────────────────────────┘
```

---

## 12. Remote Display

For MVP use:

### VNC + x11vnc + noVNC

Architecture:

```text
Remote VM

XFCE
 │
 ▼
X Server
 │
 ▼
x11vnc
 │
 ▼
WebSocket
 │
 ▼
noVNC
 │
 ▼
Tauri Application
```

The graphical desktop should appear inside the desktop application rather than opening an unrelated external VNC application.

---

## 13. Remote Terminal

The desktop application should contain an integrated terminal.

Use:

### xterm.js

Architecture:

```text
┌──────────────┐
│   xterm.js   │
└──────┬───────┘
       │
   WebSocket
       │
       ▼
┌──────────────┐
│ Remote Agent │
└──────┬───────┘
       │
      PTY
       │
       ▼
      bash
```

The user should be able to execute arbitrary commands on the remote machine.

Example:

```text
$ cd /workspace/my-app
$ npm install
$ npm run dev
```

---

## 14. Remote Agent

A lightweight agent should run on the VM.

### Technology

Go

### Name

`rental-agent`

The agent establishes an outbound connection to the backend.

Initial responsibilities:

- identify machine
- report readiness
- maintain connection
- create terminal sessions
- manage PTYs
- receive commands
- report basic machine status
- initiate graceful shutdown

The agent should **not** contain cloud-provider logic.

---

## 15. Project Transfer

The MVP should not attempt real-time filesystem synchronization.

When the rental starts:

```text
Local repository
       ↓
Archive
       ↓
Upload
       ↓
Remote VM
       ↓
/workspace/my-app
```

Use:

- tar
- gzip/zstd
- HTTP upload

For the initial prototype, even a ZIP archive is acceptable.

The goal is to prove the remote development environment, not synchronization.

---

## 16. Workspace

The remote project should be placed at:

```text
/workspace/<project-name>
```

Example:

```text
/workspace/my-app
```

The user should start their terminal in this directory.

---

## 17. Runtime Networking

The MVP needs basic outbound Internet connectivity from the VM.

This allows the developer to:

```text
npm install
pip install
git clone
docker pull
curl
etc.
```

Inbound access to the VM should **not** be exposed unnecessarily.

The desktop application should communicate through authenticated connections.

---

## 18. Backend

Use:

### Go

Responsibilities:

```text
Backend
│
├── Rental API
├── AWS EC2 management
├── Machine state
├── Agent connections
├── Terminal WebSockets
├── Project upload
└── Remote display connection
```

Initial API:

```text
POST   /rentals
GET    /rentals/:id
POST   /rentals/:id/stop

POST   /projects/upload

WS     /rentals/:id/terminal
WS     /rentals/:id/agent
```

---

## 19. Rental Management

A rental represents one temporary machine.

Minimal model:

```text
Rental
├── ID
├── Status
├── EC2 Instance ID
├── Machine Profile
├── Project Name
├── Created At
├── Started At
└── Stopped At
```

For the MVP, this can initially be held in memory.

PostgreSQL is not required until persistence becomes necessary.

---

## 20. Stop Renting

The user sees a persistent control:

> **STOP RENTING**

When pressed:

```text
STOP RENTING
      ↓
Stop new operations
      ↓
Disconnect terminal
      ↓
Disconnect display
      ↓
Terminate EC2 instance
      ↓
Mark rental RELEASED
```

The application should explicitly confirm:

> Stop renting? Your remote machine will be released.

For MVP, any unsaved remote changes can be discarded.

---

## 21. Rental Screen

Once ready:

```text
┌──────────────────────────────────────────────┐
│ my-app                       ● RENTING       │
│                                              │
│ AWS EC2                                     │
│ 8 CPU · 32 GB                               │
│                                              │
│ Rental time: 23m 14s                        │
│                                              │
├──────────────────────────────────────────────┤
│                                              │
│              REMOTE DESKTOP                  │
│                                              │
│       ┌──────────────────────────────┐       │
│       │                              │       │
│       │         Ubuntu XFCE          │       │
│       │                              │       │
│       │                              │       │
│       └──────────────────────────────┘       │
│                                              │
├──────────────────────────────────────────────┤
│ TERMINAL                                     │
│                                              │
│ $ npm run dev                                │
│ Server running on localhost:3000             │
│ $ █                                          │
│                                              │
├──────────────────────────────────────────────┤
│                         [ STOP RENTING ]     │
└──────────────────────────────────────────────┘
```

---

## 22. Error Handling

### VM provisioning failure

Show:

> Unable to start machine.

Terminate any partially-created resources.

### Agent unavailable

Show:

> Machine started but environment is not ready.

Retry connection.

### Display unavailable

Allow terminal access while attempting to reconnect graphical display.

### Stop failure

Retry termination and report the rental status.

The backend must have cleanup logic so failed provisioning doesn't leave orphaned instances running.

---

## 23. Security — MVP

Even though this is a prototype, basic security is required.

### AWS

Use an IAM role for EC2 rather than embedding AWS credentials in the VM.

### Backend

Never expose AWS credentials to the desktop application.

### VM

Only expose the minimum required network ports.

### Agent

Authenticate its connection to the backend using a short-lived rental token.

### Project

Project uploads should use authenticated, temporary upload credentials where possible.

---

## 24. MVP Technology Stack

| Layer | Technology |
|---|---|
| Desktop | Tauri |
| Frontend | React |
| Language | TypeScript |
| Native desktop | Rust |
| Backend | Go |
| API | HTTP/REST |
| Real-time | WebSocket |
| Cloud | AWS |
| Compute | EC2 |
| Storage | EBS |
| OS | Ubuntu |
| Desktop | XFCE |
| VNC server | x11vnc |
| VNC client | noVNC |
| Terminal | xterm.js |
| Remote agent | Go |
| PTY | Go PTY library |
| VM image | Packer |
| VM initialization | cloud-init |
| Project transfer | HTTP + archive |
| Database | None initially / in-memory |

---

## 25. MVP Architecture

```text
                         ┌─────────────────────┐
                         │    Tauri Desktop    │
                         │                     │
                         │ React / TypeScript  │
                         │                     │
                         │ ┌─────────────────┐ │
                         │ │     noVNC       │ │
                         │ └─────────────────┘ │
                         │                     │
                         │ ┌─────────────────┐ │
                         │ │    xterm.js     │ │
                         │ └─────────────────┘ │
                         └──────────┬──────────┘
                                    │
                         HTTPS / WebSocket
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │      Go Backend     │
                         │                     │
                         │ Rental Manager      │
                         │ AWS Manager         │
                         │ Agent Manager       │
                         │ Upload API          │
                         └──────────┬──────────┘
                                    │
                              AWS SDK v2
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │       AWS EC2       │
                         │                     │
                         │ Ubuntu              │
                         │ XFCE                │
                         │ x11vnc              │
                         │ rental-agent        │
                         │                     │
                         │ /workspace/my-app   │
                         └─────────────────────┘
```

---

## 26. Development Milestones

### Milestone 1 — Local UI

Build:

- Tauri application
- project picker
- machine selection
- Start Renting button
- rental screen

No cloud yet.

---

### Milestone 2 — AWS provisioning

Implement:

```text
Start Renting
     ↓
Go backend
     ↓
EC2 API
     ↓
Ubuntu VM
```

The backend should be able to create and terminate a VM reliably.

---

### Milestone 3 — Remote agent

Install the agent in the VM.

Verify:

```text
VM
 ↓
Agent
 ↓
Backend
```

The backend knows:

> Machine is ready.

---

### Milestone 4 — Terminal

Implement:

```text
Tauri
 ↓
xterm.js
 ↓
WebSocket
 ↓
Agent
 ↓
PTY
 ↓
bash
```

At this stage you should be able to remotely operate the VM entirely from the desktop app.

---

### Milestone 5 — Graphical desktop

Install:

```text
XFCE
x11vnc
noVNC
```

Embed noVNC in Tauri.

Goal:

> See and interact with the remote Linux desktop.

---

### Milestone 6 — Local repository

Implement:

```text
Choose Folder
      ↓
Archive
      ↓
Upload
      ↓
Remote VM
      ↓
/workspace/project
```

---

### Milestone 7 — Complete rental loop

Validate:

```text
Choose Project
      ↓
START RENTING
      ↓
VM created
      ↓
Project uploaded
      ↓
Remote desktop available
      ↓
Terminal available
      ↓
Run application
      ↓
STOP RENTING
      ↓
VM terminated
```

---

## 27. MVP Success Criteria

The MVP is successful if a developer can perform the entire flow without interacting with AWS directly.

### Target

From:

> **Start Renting**

to:

> **Interactive remote desktop**

in **under 3 minutes** for the initial implementation.

And:

> **Stop Renting**

must reliably release the EC2 compute resource.

The user should never need to:

- open AWS console
- SSH into the VM
- configure VNC
- install XFCE
- install development tools
- configure networking

---

## 28. Future Architecture Direction

Once this MVP works, the next layers can be added without changing the basic UX:

```text
                    START RENTING
                          │
                          ▼
                  Hardware Broker
                          │
             ┌────────────┼────────────┐
             ▼            ▼            ▼
            AWS          GCP        GPU Cloud
             │            │            │
             └────────────┼────────────┘
                          ▼
                    Remote Agent
                          │
              ┌───────────┼───────────┐
              ▼           ▼           ▼
           Terminal     Desktop     Devices
                                      │
                              ┌───────┴───────┐
                              ▼               ▼
                           Android           iOS
```

But none of this belongs in the first MVP.

---

## 29. Core Technical Hypotheses

The MVP is fundamentally testing three things:

### Hypothesis 1 — Remote GUI is usable

Can a developer comfortably interact with a remote graphical Linux environment from a desktop application?

### Hypothesis 2 — Provisioning can be fast enough

Can a new machine become usable within a few minutes?

### Hypothesis 3 — The rental abstraction feels natural

Does:

**Start Renting → Work → Stop Renting**

feel simpler and more useful than asking developers to manage cloud VMs themselves?

If these three hypotheses are validated, we can then invest in the much larger pieces: faster display protocols, persistent workspaces, Android/iOS, multiple providers, provider-level price optimization, and real billing.

---

## 30. Core MVP Principle

The first implementation should be intentionally boring:

**Tauri + Go + EC2 + Ubuntu + XFCE + x11vnc + noVNC + xterm.js.**

The goal is not to build a cloud platform.

The goal is to prove that a developer can:

> **Select a local project → Start Renting → get a powerful graphical remote computer → work → Stop Renting.**
