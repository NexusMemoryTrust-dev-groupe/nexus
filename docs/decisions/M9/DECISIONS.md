# M9 Desktop Interface — DECISIONS

## D049: Zustand for State Management
- Date: 2026-07-23
- Decision: Use Zustand for global state management (memoryStore, graphStore, uiStore)
- Reason: Lightweight (1KB gzipped), simple API, no boilerplate, works well with React 19. Chosen per 01_CONSTITUTION.md stack.

## D050: Tauri IPC with Result<T, String>
- Date: 2026-07-23
- Decision: Tauri commands return Result<T, String> instead of Result<T> (AppError)
- Reason: Tauri 2.0 IPC requires error type to implement specific traits. AppError doesn't implement them. Using String error type for IPC boundary; conversion happens in command handlers.

## D051: Dark Mode via CSS Classes
- Date: 2026-07-23
- Decision: Dark mode implemented via Tailwind `dark:` prefix classes
- Reason: Tailwind CSS 4 native dark mode support. No JS-based theme switching needed for MVP. System preference detection via `dark:` variant.

## D052: Command Bar with Ctrl+K Shortcut
- Date: 2026-07-23
- Decision: Command bar toggled via Ctrl+K keyboard shortcut
- Reason: Standard pattern (VS Code, Slack, Linear). Discoverable, keyboard-first UX.

## D053: Dual Mode (Explorer/Operator)
- Date: 2026-07-23
- Decision: App has two modes: Explorer (read-only analysis) and Operator (actions)
- Reason: Per 03_ARCHITECTURE_M1_M28.md M9 spec. Separates concerns: analysis vs execution.

## D054: GraphView as Placeholder
- Date: 2026-07-23
- Decision: GraphView renders placeholder div; @antv/g6 initialization deferred to integration
- Reason: @antv/g6 requires actual graph data to render. Placeholder prevents errors when no data. Full visualization will be wired during MVP Integration phase.

## D055: AiCoPilot as Standalone Panel
- Date: 2026-07-23
- Decision: AiCoPilot is a separate panel on the right side, not integrated into main content
- Reason: Per 03_ARCHITECTURE_M1_M28.md. AI Co-Pilot is a supplementary feature that shouldn't dominate the interface. Hidden on smaller screens (hidden lg:block).

## D056: Layout with Sidebar + TopBar + StatusBar
- Date: 2026-07-23
- Decision: Classic desktop layout: collapsible sidebar, top bar with mode toggle, bottom status bar
- Reason: Per M9 spec. Familiar desktop pattern. Sidebar collapsible for screen real estate.
