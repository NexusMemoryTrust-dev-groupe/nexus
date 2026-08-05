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

## D054: GraphView — собственный 3D-рендер вместо @antv/g6
- Date: 2026-07-23 (обновлено)
- Decision: Вместо placeholder'а с @antv/g6 реализован `CosmicGraphView` (Three.js): космическая сцена, LOD-система (24×24/12×12/8×8 сегментов, отсечение за порогом видимости, бюджет подписей/рёбер), поиск с группировкой по типам, контекстное меню.
- Reason: Расширение M3-данных: рендер — отдельный слой поверх `GraphStore`, без изменения API графа. LOD позволяет держать плавность на тысячах узлов.

## D055: AiCoPilot as Standalone Panel
- Date: 2026-07-23 (обновлено)
- Decision: AiCoPilot — отдельная панель справа (`FloatingCopilot`), не интегрирована в основной контент.
- Reason: AI Co-Pilot — вспомогательная функция, не должна доминировать в интерфейсе. Панель реализована полностью: стриминг, thinking-блоки, выбор модели, 66 MCP-инструментов, slash-команды. Скрывается на узких экранах (hidden lg:block).

## D056: Layout with Sidebar + TopBar + StatusBar
- Date: 2026-07-23
- Decision: Classic desktop layout: collapsible sidebar, top bar with mode toggle, bottom status bar
- Reason: Per M9 spec. Familiar desktop pattern. Sidebar collapsible for screen real estate.
