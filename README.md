# Nexus Memory Trust

<p align="center">
  <strong>AI Memory Operating System — desktop-first application for intelligent memory management</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react" alt="React">
  <img src="https://img.shields.io/badge/Tauri-2.0-24C8DB?logo=tauri" alt="Tauri">
  <img src="https://img.shields.io/badge/SQLite-embedded-003B57?logo=sqlite" alt="SQLite">
  <img src="https://img.shields.io/badge/TypeScript-5.5-3178C6?logo=typescript" alt="TypeScript">
</p>

---

## What is Nexus?

Nexus is a **desktop-first AI memory operating system** built for individuals and teams who need structured, searchable, and trustworthy memory management. It combines:

- **Structured memory capture** — organize thoughts by layer, importance, and project space
- **Knowledge graph** — visual relationships between memories, entities, and decisions
- **AI copilot** — streaming chat with real-time thinking display, powered by OpenCode
- **Version history** — automatic commits with diff tracking and rollback
- **Semantic search** — keyword-vector fingerprinting for fast memory retrieval
- **Cyberpunk timeline** — visual history with animated glow effects and date dividers

## Screenshots

> Screenshots coming soon. See the project in action after following the installation guide below.

---

## Tech Stack

| Layer | Technology |
|---|---|
| **Backend** | Rust 2024 + Tauri 2.0 |
| **Frontend** | React 19 + TypeScript 5.5 + Vite 6 |
| **Styling** | Tailwind CSS 4 + CSS custom properties |
| **AI Integration** | OpenCode CLI (streaming JSONL with `--thinking`) |
| **Database** | SQLite (embedded, versioned migrations) |
| **3D Visualization** | Three.js + @react-three/fiber |
| **Rich Text Editor** | TipTap (Markdown export) |
| **State Management** | Zustand |
| **Animations** | Framer Motion |
| **Testing** | cargo test, vitest |

---

## Project Structure

```
nexus/
├── .github/
│   └── workflows/
│       └── ci.yml                 # CI pipeline (Rust + Frontend)
├── src-tauri/                     # Rust backend (Tauri 2.0)
│   ├── src/
│   │   ├── main.rs                # Entry point + Tauri command registration
│   │   ├── commands/              # Tauri IPC commands (ai.rs, config.rs, memory.rs, ...)
│   │   ├── core/                  # Business logic (no storage/infra dependencies)
│   │   ├── storage/
│   │   │   └── sqlite/
│   │   │       ├── schema.rs      # Versioned migration runner
│   │   │       └── migrations/    # Numbered SQL files (V1..V9)
│   │   └── infra/                 # Infrastructure adapters
│   ├── benches/                   # Criterion benchmarks
│   ├── tests/                     # Integration tests
│   ├── system_rules.md            # AI security rules (compiled into binary)
│   └── Cargo.toml
├── src/                           # React frontend
│   ├── components/
│   │   ├── ai/                    # FloatingCopilot, thinking indicator
│   │   ├── layout/                # Sidebar, NexusLogo
│   │   ├── settings/              # SettingsView (model selector)
│   │   ├── timeline/              # Cyberpunk timeline view
│   │   └── graph/                 # 3D knowledge graph
│   ├── styles/
│   │   └── globals.css            # All styles including timeline + copilot
│   └── App.tsx
├── ai/                            # Python AI layer (optional, PyO3)
├── package.json
└── README.md
```

---

## Installation

### Prerequisites

| Tool | Version | Notes |
|---|---|---|
| **Rust** | 1.75+ (edition 2024) | `rustup update` |
| **Node.js** | 20+ | LTS recommended |
| **npm** | 9+ | Ships with Node.js |
| **OpenCode CLI** | Latest | `npm install -g opencode` — required for AI features |
| **Tauri CLI** | 2.x | `cargo install tauri-cli` |

### Step-by-Step

```bash
# 1. Clone the repository
git clone https://github.com/NexusMemoryTrust-dev-groupe/nexus.git
cd nexus

# 2. Install frontend dependencies
npm install

# 3. Build the frontend (TypeScript check + Vite build)
npm run build

# 4. Build the Tauri desktop app
cargo tauri build

# 5. Or run in development mode
cargo tauri dev
```

### Development Mode

```bash
# Terminal 1: Frontend dev server (hot reload)
npm run dev

# Terminal 2: Tauri backend (watches Rust changes)
cargo tauri dev
```

---

## Usage

### Creating Memories

1. Open the sidebar and click **+ New Memory**
2. Enter a title and content (Markdown supported via TipTap editor)
3. Assign a **layer** (Raw → Refined → Synthesized → Archived)
4. Set importance and confidence scores
5. Link to entities in your knowledge graph

### AI Copilot

1. Click the floating copilot button (bottom-right corner)
2. Type your question in any language (RU, EN, etc.)
3. Watch the AI think in real-time (expandable thinking indicator)
4. The copilot responds in **the same language you write in**

> **Security**: The AI will not reveal its tech stack, architecture, database schema, API keys, or implementation details. This is enforced by compiled-in security rules.

### Knowledge Graph

1. Navigate to the **Graph** view in the sidebar
2. Entities and relationships are rendered in 3D
3. Click nodes to view details and linked memories
4. Drag to rotate, scroll to zoom

### Timeline

1. Navigate to the **Timeline** view
2. Browse memory history with animated date dividers
3. Each layer pulses with its native color
4. Glass-effect cards with stagger animations

### Model Selection

1. Open **Settings** from the sidebar
2. Scroll to the **AI** section
3. Click **Refresh Models** to fetch available models from OpenCode
4. Toggle **FREE only** to filter free-tier models
5. Select your preferred model from the dropdown

---

## Architecture Rules

The codebase follows strict architectural principles:

- **Clean Architecture**: `core/` has no dependencies on `storage/`, `infra/`, or Tauri
- **Error handling**: All business logic returns `Result<T>`, no panics
- **DI through traits**: Core never imports concrete implementations
- **SOLID principles** enforced across all layers
- **Security**: AI rules compiled into binary via `include_str!`

### Database Migrations

Migrations use a versioned file system:

```
src-tauri/src/storage/sqlite/migrations/
├── V1_create_memory_records.sql
├── V2_add_attached_files.sql
├── V3_add_versioning_columns.sql
├── V4_create_versioning_tables.sql
├── V5_create_entity_snapshots.sql
├── V6_create_graph_tables.sql
├── V7_create_context_tables.sql
├── V8_create_workspace_and_links.sql
└── V9_create_semantic_fingerprints.sql
```

Each migration is tracked in `schema_migrations` table. The system supports:
- **Forward migration**: `apply_migrations(conn)`
- **Rollback**: `rollback_last_migration(conn)` (safe for additive migrations)
- **Idempotency**: Running `apply_migrations` twice is safe

To add a new migration:
1. Create `V{N}_description.sql` in the `migrations/` directory
2. Add the entry to `MIGRATIONS` array in `schema.rs`

---

## Testing

### Run All Tests

```bash
# Rust unit + integration tests
cd src-tauri
cargo test

# Frontend unit tests
cd ..
npm test

# Benchmarks
cargo bench
```

### CI Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs on every push/PR:

| Job | What it checks |
|---|---|
| **rust-tests** | `cargo fmt --check`, `cargo clippy`, `cargo build`, `cargo test` |
| **frontend-tests** | `npm ci`, `tsc --noEmit`, `npm run lint`, `npm test`, `npm run build` |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

### Quick Start

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes following the architecture rules
4. Run tests: `cargo test && npm test`
5. Submit a pull request

### Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add semantic search endpoint
fix: resolve timeline date divider alignment
docs: update installation guide
refactor: extract graph rendering into separate module
```

---

## Security

Nexus enforces strict security rules for AI interactions:

- **No tech stack disclosure** — the AI will not reveal it runs on Rust/React/SQLite
- **No architecture details** — internal structure, modules, and patterns are confidential
- **No code exposure** — source code, imports, and implementations are protected
- **No database schema leaks** — table structures and relationships are hidden
- **No API key exposure** — credentials and configuration are never discussed
- **Language matching** — AI always responds in the same language the user writes in

These rules are compiled into the binary at build time via `include_str!("../../system_rules.md")`.

---

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- [Tauri](https://tauri.app/) — Desktop app framework
- [OpenCode](https://opencode.ai/) — AI integration layer
- [Three.js](https://threejs.org/) — 3D visualization
- [TipTap](https://tiptap.dev/) — Rich text editing
- [Framer Motion](https://www.framer.com/motion/) — Animations

---

<p align="center">
  Built with care by the Nexus Memory Trust team
</p>
