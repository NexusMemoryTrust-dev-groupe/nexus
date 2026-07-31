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

## Download & Install

### Windows

1. Download **Nexus-Setup-x64.exe** from [GitHub Releases](https://github.com/NexusMemoryTrust-dev-groupe/nexus/releases)
2. Run the installer
3. Follow the setup wizard
4. Launch Nexus from desktop shortcut

### Linux

1. Download **.deb** or **.AppImage** from [GitHub Releases](https://github.com/NexusMemoryTrust-dev-groupe/nexus/releases)
2. Install with your package manager or run the AppImage
3. Launch Nexus from applications menu

**No additional software required.** The installer handles everything.

---

## First Launch

When you open Nexus for the first time:

1. **Configure AI API key** — the wizard will ask for your API key (OpenAI, Anthropic, Google, or OpenRouter)
2. **Select AI model** — choose your preferred model (free models available)
3. **Start creating memories** — click **+ New Memory** in the sidebar

Your API key is stored locally and never sent to our servers.

---

## Features

### Creating Memories

1. Open the sidebar and click **+ New Memory**
2. Enter a title and content (Markdown supported)
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
3. Click **Refresh Models** to fetch available models
4. Toggle **FREE only** to filter free-tier models
5. Select your preferred model from the dropdown

---

## MCP Server (for AI Assistants)

Nexus includes a built-in **MCP server** that lets any AI assistant (Claude Desktop, Cursor, Continue, Windsurf, etc.) read and write your memories, query the knowledge graph, and run commands.

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

> Replace `C:\path\to\nexus.exe` with the actual path to your Nexus installation.

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

---

## Security

Nexus enforces strict security rules for AI interactions:

- **No tech stack disclosure** — the AI will not reveal it runs on Rust/React/SQLite
- **No architecture details** — internal structure, modules, and patterns are confidential
- **No code exposure** — source code, imports, and implementations are protected
- **No database schema leaks** — table structures and relationships are hidden
- **No API key exposure** — credentials and configuration are never discussed
- **Language matching** — AI always responds in the same language the user writes in

These rules are compiled into the binary at build time.

---

## Support

- **Issues:** [GitHub Issues](https://github.com/NexusMemoryTrust-dev-groupe/nexus/issues)
- **Email:** nexus-memory-trust@proton.me

---

## License

This project is licensed under a proprietary All-Rights-Reserved license. See [LICENSE](LICENSE) for details.

**No unauthorized copying, modification, or distribution is permitted.**

---

<p align="center">
  Built with care by the Nexus Memory Trust team
</p>
