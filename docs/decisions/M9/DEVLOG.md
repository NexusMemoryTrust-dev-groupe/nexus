# M9 Desktop Interface — DEVLOG

## 2026-07-23 M9 Implementation

### Files Created

**src/types/**
- `index.ts` — Memory, GraphNode, GraphEdge, ContextPackage, AppMode, ActiveView types

**src/stores/**
- `memoryStore.ts` — Zustand store: memories, selectedMemory, isLoading, error, fetchMemories, selectMemory
- `graphStore.ts` — Zustand store: nodes, edges, selectedNode, isLoading, fetchGraph, selectNode
- `uiStore.ts` — Zustand store: mode, sidebarOpen, commandBarOpen, activeView, toggleMode, toggleSidebar, toggleCommandBar, setActiveView

**src/components/layout/**
- `Layout.tsx` — Flex layout: Sidebar + main area (TopBar + content + StatusBar)
- `Sidebar.tsx` — Navigation with lucide-react icons, collapsible, active state highlighting
- `TopBar.tsx` — Mode toggle, command bar button with Ctrl+K hint
- `StatusBar.tsx` — Current mode, memory count, connection status

**src/components/memory/**
- `MemoryExplorer.tsx` — Grid of MemoryCards, loading/error/empty states
- `MemoryCard.tsx` — Card with title, summary, layer badge, confidence/importance scores
- `MemoryDetail.tsx` — Full detail view with all fields, back button

**src/components/graph/**
- `GraphView.tsx` — Container with ref for graph data loading; позже заменён на `CosmicGraphView` (Three.js, LOD)

**src/components/context/**
- `ContextView.tsx` — Context package display: query, intent, confidence, entities, memories, token count

**src/components/command/**
- `CommandBar.tsx` — Modal overlay with input, Ctrl+K toggle, Escape to close

**src/components/timeline/**
- `TimelineView.tsx` — Vertical timeline sorted by date, each entry shows title, date, layer

**src/components/ai/**
- `AiCoPilot.tsx` — Chat panel with input, message list; позже заменён на `FloatingCopilot` (стриминг, thinking, 66 MCP-инструментов)

**src/hooks/**
- `useTauri.ts` — Wrapper around invoke with error handling
- `useMemory.ts` — Hook that fetches memories on mount

**src/**
- `App.tsx` — Updated: Layout with conditional content based on activeView, AiCoPilot panel, CommandBar overlay

**src-tauri/src/commands/**
- `memory.rs` — get_memories, get_memory, create_memory, search_memories (wired to SqliteMemoryRepository)
- `graph.rs` — get_graph, get_entity, create_entity, link_entity_to_project (wired to SqliteGraphRepository)
- `ai.rs` — ai_health_check, ai_chat_stream, ai_list_models (wired to opencode CLI)
- `mod.rs` — Updated: pub mod memory, graph, ai, copilot, files, workspace, savings, config, setup

**Root**
- `index.html` — Entry point for Vite/Tauri WebView

### Files Updated
- `src-tauri/src/main.rs` — Added command handlers to invoke_handler
- `src-tauri/src/commands/mod.rs` — Added module declarations

### Verified
- `cargo build` ✅ — Zero errors
- `cargo test` ✅ — 270/270 tests pass
- `npx tsc --noEmit` ✅ — Zero errors
- `npx vite build` ✅ — Built in 1.49s (219.63 kB JS, 21.49 kB CSS)

### Architecture Notes
- Tauri commands use Result<T, String> for IPC compatibility
- GraphView: вместо placeholder'а реализован собственный 3D-рендер (Three.js, LOD) — см. CosmicGraphView
- AiCoPilot: полностью реализован (стриминг, thinking, выбор модели, 66 MCP-инструментов) — см. FloatingCopilot
- Dark mode через CSS-переменные дизайн-системы, не Tailwind-утилиты
- Zustand stores независимы, без cross-store зависимостей
- 32 vitest + 10 Playwright e2e покрывают UI-критичные пути
