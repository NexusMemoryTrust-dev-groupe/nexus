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
├── scripts/
│   ├── install-nexus.ps1          # Master installer (auto-installs all deps)
│   └── first-run-setup.ps1        # First-run wizard (API key + model config)
├── package.json
└── README.md
```

---

## Installation

### Quick Install (Recommended)

Run the master installer — it checks and installs everything automatically:

```powershell
# Run as Administrator in PowerShell
.\scripts\install-nexus.ps1
```

This will:
- ✓ Check and install **Node.js 20 LTS** (if missing)
- ✓ Check and install **npm** (comes with Node.js)
- ✓ Check and install **OpenCode AI CLI** (if missing)
- ✓ Validate all system requirements

### First-Run Setup

After building/installing the app, run the first-run wizard:

```powershell
.\scripts\first-run-setup.ps1
```

This will:
- ✓ Auto-detect and install missing dependencies
- ✓ Configure your **AI API key** (OpenAI, Anthropic, Google, or OpenRouter)
- ✓ Select your **default AI model** (free model available)
- ✓ Save configuration to `~/.nexus/setup.json`

### Manual Install (Developers)

| Tool | Version | Notes |
|---|---|---|
| **Rust** | 1.75+ (edition 2024) | `rustup update` |
| **Node.js** | 20+ | LTS recommended |
| **npm** | 9+ | Ships with Node.js |
| **Tauri CLI** | 2.x | `cargo install tauri-cli` |

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

### Pre-built Releases

Download the latest installer from [GitHub Releases](https://github.com/NexusMemoryTrust-dev-groupe/nexus/releases):
- **Windows**: `Nexus-Setup-x64.exe` (NSIS installer)
- **Linux**: `.deb` or `.AppImage`

The installer handles everything — no manual dependency installation required.

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

## MCP Server (Model Context Protocol)

Nexus includes a built-in **MCP server** that lets any AI assistant (Claude Desktop, Cursor, Continue, Windsurf, etc.) read and write your memories, query the knowledge graph, and run commands — all via the standard MCP protocol over stdio.

### Quick Start

```bash
# Build the app
cargo tauri build

# Run in MCP mode (stdio JSON-RPC)
nexus.exe --mcp
```

### Connect to Claude Desktop

Add this to your Claude Desktop config:

**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "nexus": {
      "command": "C:\\path\\to\\nexus.exe",
      "args": ["--mcp"]
    }
  }
}
```

> Replace `C:\path\to\nexus.exe` with the actual path to your built executable.

### Connect to Cursor

Go to **Settings → AI → MCP Servers** and add:

```json
{
  "nexus": {
    "command": "C:\\path\\to\\nexus.exe",
    "args": ["--mcp"]
  }
}
```

### Connect to Continue (VS Code / JetBrains)

Add to `~/.continue/config.json`:

```json
{
  "mcpServers": [
    {
      "name": "nexus",
      "command": "C:\\path\\to\\nexus.exe",
      "args": ["--mcp"]
    }
  ]
}
```

### Available MCP Tools (31 tools)

#### Memory CRUD

| Tool | Description | Input |
|---|---|---|
| `nexus_list_memories` | List all memory records | `{}` |
| `nexus_get_memory` | Get a memory by ID | `{ "id": "uuid" }` |
| `nexus_create_memory` | Create a new memory | `{ "title": "...", "content": "...", "author": "user" }` |
| `nexus_update_memory` | Update memory content | `{ "id": "uuid", "content": "new text" }` |
| `nexus_delete_memory` | Delete a memory | `{ "id": "uuid" }` |
| `nexus_get_recent_memories` | Get memories from last N days | `{ "days": 7 }` |
| `nexus_get_important_memories` | Get memories above importance threshold | `{ "threshold": 0.7 }` |

#### Search

| Tool | Description | Input |
|---|---|---|
| `nexus_search_memories` | Full-text search across memories | `{ "query": "search term" }` |
| `nexus_search_context` | Enhanced search with intent detection | `{ "query": "topic" }` |
| `nexus_search_semantic` | Semantic search via ONNX embeddings | `{ "query": "text", "limit": 10 }` |
| `nexus_analyze_text` | Extract keywords, entities, temporal refs | `{ "text": "analyze this" }` |

#### Entity CRUD

| Tool | Description | Input |
|---|---|---|
| `nexus_get_entity` | Get an entity by ID | `{ "id": "uuid" }` |
| `nexus_create_entity` | Create a graph entity | `{ "entity_type": "Person", "title": "Name" }` |
| `nexus_update_entity` | Update entity title | `{ "id": "uuid", "title": "New" }` |
| `nexus_delete_entity` | Delete an entity | `{ "id": "uuid" }` |
| `nexus_list_graph_entities` | List entities (optionally by type) | `{ "entity_type": "Person", "limit": 100 }` |

#### Relationships

| Tool | Description | Input |
|---|---|---|
| `nexus_link_entities` | Create entity-entity relationship | `{ "source_id": "uuid", "target_id": "uuid", "relationship_type": "RelatedTo", "weight": 0.8 }` |
| `nexus_unlink_entities` | Delete a relationship | `{ "relationship_id": "uuid" }` |
| `nexus_link_memory_entity` | Link memory to entity | `{ "memory_id": "uuid", "entity_id": "uuid", "relationship": "Related", "weight": 1.0 }` |
| `nexus_unlink_memory_entity` | Remove memory-entity link | `{ "memory_id": "uuid", "entity_id": "uuid" }` |
| `nexus_get_memory_links` | Get all entity links for a memory | `{ "memory_id": "uuid" }` |
| `nexus_get_entity_memory_links` | Get all memory links for an entity | `{ "entity_id": "uuid" }` |

#### Intelligence

| Tool | Description | Input |
|---|---|---|
| `nexus_build_context` | Build AI context (full M4 pipeline) | `{ "query": "topic" }` |
| `nexus_parse_markdown` | Parse Markdown → graph entities | `{ "text": "# Heading..." }` |
| `nexus_store_fingerprint` | Store semantic fingerprint | `{ "memory_id": "uuid", "text": "keywords source" }` |

#### System

| Tool | Description | Input |
|---|---|---|
| `nexus_stats` | Database statistics | `{}` |
| `nexus_health` | Health check | `{}` |
| `nexus_settings` | Current settings | `{}` |
| `nexus_timeline` | Entity timeline by creation date | `{}` |
| `nexus_graph_stats` | Knowledge graph stats | `{}` |
| `nexus_copilot_command` | Execute a copilot slash command | `{ "command": "/health" }` |

### Available MCP Resources

| Resource URI | Description |
|---|---|
| `nexus://stats` | Memory and entity counts |
| `nexus://health` | Database connectivity status |
| `nexus://settings` | Application configuration |

### Example: AI Assistant Using Your Memories

Once connected, your AI assistant can:

> **"Show me all memories about the API project"**
> → Calls `nexus_search_memories` with query "API project"

> **"Create a new memory: Meeting with team about Q3 planning"**
> → Calls `nexus_create_memory` with title and content

> **"What's the knowledge graph structure?"**
> → Calls `nexus_graph_stats` and `nexus_build_context`

> **"Show recent activity"**
> → Calls `nexus_get_recent_memories` with `days: 7`

> **"Find important memories"**
> → Calls `nexus_get_important_memories` with `threshold: 0.7`

### Protocol Details

- **Protocol version:** 2024-11-05 (MCP specification)
- **Transport:** stdio (JSON-RPC 2.0, one JSON object per line)
- **Server name:** `nexus-mcp-server` v1.0.0
- **No authentication required** — runs locally on your machine
- **All data stays local** — nothing is sent to external servers

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
