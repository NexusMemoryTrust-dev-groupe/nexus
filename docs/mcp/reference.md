# Справочник MCP-инструментов Nexus

Все **143** инструментов с входными схемами. Сгенерировано из
`tool_definitions()` — того же источника, что отвечает сервер на
`tools/list`.

## Память

CRUD и поиск записей памяти.

### `nexus_list_memories`

List all memory records in the Nexus database

**Входная схема**

_Без параметров._


### `nexus_get_memory`

Get a single memory record by ID

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |


### `nexus_create_memory`

Create a new memory record with title and content

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `author` | `string`, default: `"user"` | — | Author name |
| `content` | `string` | **да** | Memory content |
| `title` | `string` | **да** | Memory title |


### `nexus_update_memory`

Update an existing memory record's content

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `content` | `string` | **да** | New content |
| `id` | `string` | **да** | Memory UUID |


### `nexus_delete_memory`

Delete a memory record by ID

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |


### `nexus_search_memories`

Search memories by query string

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `query` | `string` | **да** | Search query |


### `nexus_search_semantic`

Search memories by semantic similarity using ONNX embeddings (AllMiniLML6V2, 384-dim)

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer`, default: `10` | — | Max results |
| `query` | `string` | **да** | Search query |


### `nexus_get_recent_memories`

Get recent memories from the last N days

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `days` | `integer`, default: `7` | — | Number of days to look back |


### `nexus_get_important_memories`

Get memories with importance above threshold

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `threshold` | `number`, default: `0.7` | — | Importance threshold (0.0-1.0) |


### `nexus_store_fingerprint`

Store semantic fingerprint for a memory

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `memory_id` | `string` | **да** | Memory UUID |
| `text` | `string` | **да** | Text to extract keywords from |


### `nexus_memory_score`

Nexus Memory Score — health panel of the project's memory: coverage (share of graph entities covered by memories), freshness, consistency, trust, redundancy, conflict rate and context quality (maturity of knowledge across cognitive layers), plus an overall MEMORY HEALTH percentage. Use it to answer 'how healthy is this project's brain?' and to spot where memory needs attention (stale, redundant, conflicted).

**Входная схема**

_Без параметров._


## Когнитивные слои

Шесть когнитивных слоёв и их провенанс.

### `nexus_layers_list`

List the six cognitive layers (Working, Episodic, Semantic, Procedural, Decision, Strategic) with their meaning and what promotes to them. Use this before classifying anything — the layer ladder answers 'what kind of knowledge is this?'.

**Входная схема**

_Без параметров._


### `nexus_layer_stats`

Distribution of memories across cognitive layers with mean classifier confidence per layer. Reveals the shape of the knowledge pool — where the project has facts vs decisions vs principles. Use it to answer 'what does the project actually know?'.

**Входная схема**

_Без параметров._


### `nexus_layer_set`

Explicitly assign a cognitive layer to a memory (user override). Records full provenance: confidence 1.0, reason, and a history entry tagged 'user' that pins the layer against auto-reclassification. Layer names: Working, Episodic, Semantic, Procedural, Decision, Strategic.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |
| `layer` | `string` | **да** | One of: Working, Episodic, Semantic, Procedural, Decision, Strategic |
| `reason` | `string` | — | Why this layer (optional, recorded in history) |


### `nexus_layer_reclassify`

Re-run the signature classifier on a memory and persist the result (with a history entry tagged 'classifier'). No-op if the layer is user-pinned. Use when content changed and the layer may be stale.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |


### `nexus_layer_history`

Full provenance trail of a memory's layer: every assignment (layer, confidence, reason, timestamp, by=user|classifier|migration), newest first. Answers 'why is this memory on this layer?'.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |


## Жизненный цикл памяти

Подтверждение, устаревание, конфликты, обратная связь.

### `nexus_memory_set_state`

Set the trust state of a memory explicitly: Current, Inferred, Superseded, or Conflicted. Use this to mark a memory as outdated, disputed, or re-verified.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |
| `state` | `string` | **да** | New state: Current | Inferred | Superseded | Conflicted |


### `nexus_memory_confirm`

Mark a memory as explicitly confirmed by a human. The memory state becomes UserConfirmed with a timestamp. Use this to lock in a verified fact.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `by` | `string` | — | Who confirmed it (optional) |
| `id` | `string` | **да** | Memory UUID |


### `nexus_memory_feedback`

Record user feedback on a memory: useful, irrelevant, or wrong. One vote per memory — voting the same kind again removes the vote, a different kind switches it. Optionally explain why in 'note'; the explanation is kept and used by the copilot to understand what is right or wrong about the memory. A 'wrong' verdict also marks the memory Conflicted so it stops being trusted as-is.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory UUID |
| `kind` | `string` | **да** | Feedback kind |
| `note` | `string` | — | Optional explanation of why this feedback was given |


### `nexus_memory_supersede`

Replace an outdated memory with a newer one. The old memory is marked Superseded (never deleted), and a new Current record is created with the new title/content.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `author` | `string` | — | Author of the new memory (optional) |
| `new_content` | `string` | **да** | Content of the new memory |
| `new_title` | `string` | **да** | Title of the new memory |
| `old_id` | `string` | **да** | UUID of the memory to replace |


### `nexus_lifecycle_overview`

Get the memory trust lifecycle overview: how many memories are Current, UserConfirmed, Inferred, Superseded, and Conflicted. Use this for a memory-health dashboard.

**Входная схема**

_Без параметров._


## Конфликты

Обнаружение, разбор и разрешение противоречий.

### `nexus_conflict_check`

Full conflict health check: reconcile conflict groups with the current Conflicted records (clustering duplicates into existing open groups), then report every open conflict with the engine's current verdict. One call to know 'is the knowledge pool self-consistent, and if not, what exactly contradicts what?'.

**Входная схема**

_Без параметров._


### `nexus_conflict_details`

One conflict group by id: topic, member memory ids, status, and the stored resolution (winner, confidence, reasons, by=user|engine, when). Use before resolving to see the full picture of the contradiction.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Conflict group UUID |


### `nexus_conflict_list`

List conflict groups — semantic contradictions between memories (both sides marked Conflicted). Optional status filter (open|resolved). Use this to find 'what does the project disagree with itself about?' and to surface open conflicts that need a decision.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `status` | `string` | — | Optional filter: open | resolved |


### `nexus_conflict_resolve`

Settle a conflict: the winner becomes Current (engine) or UserConfirmed (user), every loser becomes Superseded (linked back to the winner), the group is marked resolved with full resolution provenance. When the engine's confidence is below 0.70 and a human must pick, this records the human verdict.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `by` | `string` | **да** | Who decides: user | engine |
| `id` | `string` | **да** | Conflict group UUID |
| `reason` | `string` | — | Optional human reason for the resolution |
| `winnerId` | `string` | **да** | Memory id that wins the conflict |


### `nexus_conflict_truth`

Run the Current Truth Engine over a conflict's members (read-only, nothing persisted). Returns the current winner, normalized confidence (0–1) and human-readable reasons ('+ recent source', '+ user confirmation'). Use to answer 'which memory is right RIGHT NOW?' before deciding anything.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Conflict group UUID |


## Сущности — дедупликация

Поиск и слияние дубликатов графа.

### `nexus_find_duplicates`

Scan the knowledge graph for duplicate entities (exact + normalized + fuzzy name match). Returns groups of 2+ entities that look like the same thing, with a bestId merge target per group. Use this before merging to review what would be combined.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `min_score` | `number`, default: `0.78` | — | Minimum Dice similarity (default 0.78). Lower finds more (noisier) groups, higher finds only strong matches. |


### `nexus_merge_entities`

Merge duplicate entities into one canonical node. The primary is kept; every id in duplicates is merged into it (metadata combined, relationships redirected, duplicates marked Merged). Idempotent.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `duplicates` | `array` | **да** | UUIDs of the entities to merge into primary |
| `primary` | `string` | **да** | UUID of the entity to keep (use bestId from nexus_find_duplicates) |


## Граф знаний

Сущности и связи.

### `nexus_graph_stats`

Get knowledge graph statistics (entity counts by type)

**Входная схема**

_Без параметров._


### `nexus_get_entity`

Get a single entity by ID

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Entity UUID |


### `nexus_create_entity`

Create a new entity in the knowledge graph

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_type` | `string` | **да** | Entity type (Person, Organization, Project, Document, Meeting, Decision, Task, Technology, Memory) |
| `title` | `string` | **да** | Entity title |


### `nexus_update_entity`

Update an existing entity's title

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Entity UUID |
| `title` | `string` | **да** | New title |


### `nexus_delete_entity`

Delete an entity by ID

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Entity UUID |


### `nexus_link_entities`

Create a relationship between two entities

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `relationship_type` | `string`, default: `"RelatedTo"` | — | Relationship type (Uses, DependsOn, CreatedBy, RelatedTo, Implements, etc.) |
| `source_id` | `string` | **да** | Source entity UUID |
| `target_id` | `string` | **да** | Target entity UUID |
| `weight` | `number`, default: `0.8` | — | Relationship weight (0.0-1.0) |


### `nexus_unlink_entities`

Delete a relationship by ID

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `relationship_id` | `string` | **да** | Relationship UUID |


### `nexus_list_graph_entities`

List all graph entities, optionally filtered by type

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_type` | `string` | — | Filter by entity type (Person, Organization, Project, Document, Meeting, Decision, Task, Technology, Memory) |
| `limit` | `integer`, default: `100` | — | Max results to return |


### `nexus_entity_metadata`

Get the metadata map of an entity (key/value pairs stored on the entity). Returns an empty object if the entity has no metadata.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Entity UUID |


### `nexus_link_project_entity`

Link an entity to a project by creating a relationship (default type: Uses). Use this to attach documents, people, decisions and other entities to a project.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_id` | `string` | **да** | Entity UUID to link to the project |
| `project_id` | `string` | **да** | Project entity UUID |
| `relationship_type` | `string`, default: `"Uses"` | — | Relationship type (default: Uses) |
| `weight` | `number`, default: `0.8` | — | Relationship weight (default: 0.8) |


## Контекст и поиск

Сборка пакета контекста и разбор текста.

### `nexus_build_context`

Build a context package for a query (full M4 pipeline)

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `query` | `string` | **да** | Context query |


### `nexus_build_context_for_entity`

Build a context package centered on a specific entity with configurable traversal depth

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `depth` | `integer`, default: `2` | — | Traversal depth (1=hops only, 2=hops of hops, default=2) |
| `entity_id` | `string` | **да** | Entity UUID |


### `nexus_search_context`

Enhanced context search with intent detection, keywords, and temporal reasoning

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `query` | `string` | **да** | Search query with optional temporal references |


### `nexus_analyze_text`

Analyze text to extract keywords, entities, and temporal references

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `text` | `string` | **да** | Text to analyze |


### `nexus_parse_markdown`

Parse markdown text and extract entities and relationships (Auto Graph Builder)

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `text` | `string` | **да** | Markdown text to parse |


## Связи память ↔ сущность

Привязка записей к сущностям.

### `nexus_link_memory_entity`

Link a memory to an entity

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_id` | `string` | **да** | Entity UUID |
| `memory_id` | `string` | **да** | Memory UUID |
| `relationship` | `string`, default: `"Related"` | — | Relationship type |
| `weight` | `number`, default: `1` | — | Link weight (0-1) |


### `nexus_unlink_memory_entity`

Remove link between a memory and an entity

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_id` | `string` | **да** | Entity UUID |
| `memory_id` | `string` | **да** | Memory UUID |
| `relationship` | `string`, default: `"Related"` | — | Relationship type |


### `nexus_get_memory_links`

Get all entity links for a memory

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `memory_id` | `string` | **да** | Memory UUID |


### `nexus_get_entity_memory_links`

Get all memory links for an entity

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_id` | `string` | **да** | Entity UUID |


## Проекты

Рабочие проекты и их содержимое.

### `nexus_projects`

List all projects (entities with type Project) in the knowledge graph. Use this to enumerate projects and get their IDs for workspace/project-scoped operations.

**Входная схема**

_Без параметров._


### `nexus_project_entities`

Get all entities linked to a project via relationships, plus the relationships themselves. Use this to see what a project contains (documents, people, decisions, etc.).

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `project_id` | `string` | **да** | Project entity UUID |


### `nexus_project_memories`

Get all memory records linked to a specific project. Use this to list the memories saved in a project's space.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `project_id` | `string` | **да** | Project entity UUID |


## Рабочая область

Пространство файлов проекта.

### `nexus_add_to_workspace`

Add native file(s)/folder(s) to a project workspace. Scans directories recursively and registers all files.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `paths` | `array` | **да** | Native paths to add (files or folders) |
| `project_id` | `string` | **да** | Project entity UUID |


### `nexus_get_workspace`

Get the workspace file tree for a project

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `project_id` | `string` | **да** | Project entity UUID |


### `nexus_sync_workspace`

Sync workspace: rescan root dirs, remove stale entries, add new files from disk

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `project_id` | `string` | **да** | Project entity UUID |


### `nexus_workspace_check_stale`

Check all projects for stale folders — returns the list of project_ids whose ALL workspace root directories no longer exist on disk. Use this to detect dead projects before cleanup.

**Входная схема**

_Без параметров._


### `nexus_workspace_rename`

Rename a workspace entry (file or folder) — renames on disk AND updates the workspace database, including all children. Returns the new absolute path.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `new_name` | `string` | **да** | New name (file/folder name only, not a full path) |
| `old_path` | `string` | **да** | Current absolute path of the entry |
| `project_id` | `string` | **да** | Project entity UUID |


### `nexus_workspace_move`

Move a workspace entry (file or folder) to another directory — moves on disk (with cross-filesystem fallback) AND updates the workspace database. Returns the new absolute path.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `dest_dir` | `string` | **да** | Absolute path of the destination directory |
| `project_id` | `string` | **да** | Project entity UUID |
| `source_path` | `string` | **да** | Absolute path of the entry to move |


### `nexus_workspace_delete`

Delete a workspace entry (file or folder) — deletes from disk AND removes it from the workspace database (including all descendants). Irreversible.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `file_path` | `string` | **да** | Absolute path of the entry to delete |
| `project_id` | `string` | **да** | Project entity UUID |


### `nexus_workspace_remove`

Remove an entry from the workspace database ONLY — does NOT delete the file/folder from disk. Use this to un-register a file from a project without touching the filesystem.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `file_path` | `string` | **да** | Absolute path of the entry to remove from the workspace DB |
| `project_id` | `string` | **да** | Project entity UUID |


## Файлы (песочница)

Файловые операции внутри whitelist-корней.

### `nexus_index_file`

Index a file into the knowledge graph: reads content, extracts entities (classes, functions, headings, etc.), creates Document entity + sub-entities with relationships. Supports: py, js, ts, rs, go, java, c, cpp, md, json, yaml, toml, html, css, images.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Absolute file path |
| `project_id` | `string` | — | Optional: Project entity UUID to link file to |


### `nexus_index_folder`

Index all interpretable files in a folder recursively into the knowledge graph. Skips hidden dirs, target/, node_modules/, __pycache__/.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Absolute folder path |
| `project_id` | `string` | — | Optional: Project entity UUID to link files to |


### `nexus_scan_folder`

Scan a folder on disk and return its file tree (FileEntry with children). Use this to inspect a directory before indexing or linking it to a project.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `folder_path` | `string` | **да** | Absolute path of the folder to scan |


### `nexus_read_file_content`

Read and interpret file content: returns summary, extracted entities, and raw text. Does NOT create entities in the graph — use nexus_index_file for that.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Absolute file path |


### `nexus_read_file`

Read raw file content as text. Returns the file content without interpretation or entity extraction. Use for reading code, config files, etc.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Absolute file path to read |


### `nexus_create_file`

Create a new file on disk with content. Creates parent directories automatically. Fails if file already exists — use nexus_write_file to overwrite.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `content` | `string` | **да** | File content to write |
| `path` | `string` | **да** | Absolute file path to create |


### `nexus_write_file`

Write content to a file. Creates file if it doesn't exist, overwrites if it does. Creates parent directories automatically.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `content` | `string` | **да** | Content to write |
| `path` | `string` | **да** | Absolute file path to write |


### `nexus_create_folder`

Create a directory (and all parent directories) on disk.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Absolute directory path to create |


### `nexus_delete_file`

Delete a file or directory (recursive for directories). Use with caution.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Absolute path to delete |


### `nexus_delete_folder`

Recursively delete a folder on disk (not tied to a workspace project). Irreversible.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `folder_path` | `string` | **да** | Absolute path of the folder to delete |


### `nexus_rename_file`

Rename a file or folder on disk (not tied to a workspace project). Returns the new absolute path.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `new_name` | `string` | **да** | New name (file/folder name only, not a full path) |
| `old_path` | `string` | **да** | Current absolute path |


### `nexus_move_file`

Move or rename a file/directory. Provide either new_path (full destination) or dest_dir + new_name.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `dest_dir` | `string` | — | Destination directory (for move) |
| `new_name` | `string` | — | New name (used with dest_dir) |
| `new_path` | `string` | — | New full destination path (for rename/move) |
| `source_path` | `string` | **да** | Source file/directory path |


### `nexus_create_workspace_file`

Create a file in a project workspace — creates on disk AND registers in the workspace database. Use this when an AI wants to save generated code into a Nexus project.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `content` | `string` | **да** | File content to write |
| `name` | `string` | **да** | File name (e.g. 'main.rs', 'index.ts') |
| `parent_path` | `string` | **да** | Absolute path of parent directory in workspace |
| `project_id` | `string` | **да** | Project entity UUID |


## Экономия и продуктовые метрики

Токены, сэкономленные средства, метрики ценности.

### `nexus_savings_stats`

Get cumulative token and cost savings statistics: total tokens saved, cost saved (USD), per-day/week/month/year breakdown, average per interaction, and recent interactions. Real data from the database — no estimates.

**Входная схема**

_Без параметров._


### `nexus_savings_report`

Get a full savings report: aggregate stats PLUS per-model cost breakdown for all 21 supported LLMs (how much the saved tokens would have cost with each model's input price). Use this to answer 'how much money did Nexus save me?'

**Входная схема**

_Без параметров._


### `nexus_savings_per_model`

Calculate savings for a specific LLM model: how much the saved tokens would have cost with that model's input price. Model names are case-insensitive, e.g. 'GPT-5.6 Terra', 'deepseek v4 flash', 'Claude Opus 5'.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `model` | `string` | **да** | Model display name (e.g. 'GPT-5.6 Terra', 'DeepSeek V4 Flash') |


### `nexus_savings_record`

Record a measured token-savings event manually (baseline vs context usage). Use this to log a savings measurement that the UI would normally record automatically.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `baseline_tokens` | `integer` | **да** | Baseline token count |
| `candidate_entities` | `integer`, default: `0` | — | Candidate entities filtered |
| `candidate_memories` | `integer`, default: `0` | — | Candidate memories filtered |
| `context_tokens` | `integer` | **да** | Context token count |
| `entities_count` | `integer`, default: `0` | — | Entities included |
| `intent_type` | `string`, default: `"unknown"` | — | Detected intent type |
| `irrelevant_fragments` | `integer`, default: `0` | — | Fragments dropped as below the relevance floor |
| `latency_ms` | `integer`, default: `0` | — | Context build latency in milliseconds |
| `manual_context` | `integer`, default: `0` | — | 1 if the user added context manually this round |
| `memories_count` | `integer`, default: `0` | — | Memories included |
| `precision` | `number`, default: `0` | — | Precision of collected context (included/considered), 0..1 |
| `query` | `string`, default: `""` | — | Query text |
| `relationships_count` | `integer`, default: `0` | — | Relationships included |
| `used_fragments` | `integer`, default: `0` | — | Fragments actually used in the final answer |


### `nexus_product_metrics`

Get product metrics that prove Nexus' value: share of queries without manual context, average context precision, used/irrelevant fragments, token savings vs baseline, average build latency, stale memories, memory fixes, and cross-session memory reuse. Use this to answer 'does Nexus actually help?' with measured data.

**Входная схема**

_Без параметров._


## Система и диагностика

Здоровье, статистика, настройки, конфигурация.

### `nexus_copilot_command`

Execute a Nexus copilot slash command. Supported: /memories, /memory <id>, /create-memory <title> <content>, /update-memory <id> <content>, /delete-memory <id>, /search <query>, /graph, /entity <id>, /create-entity <type> <title>, /update-entity <id> <title>, /delete-entity <id>, /link <source_id> <target_id> [type] [weight], /unlink <rel_id>, /context <query>, /entity_context <id> [depth], /stats, /health, /settings, /timeline, /savings, /savings-model <model_name>, /help, /projects

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `command` | `string` | **да** | The slash command to execute |


### `nexus_stats`

Show database statistics (memory and entity counts)

**Входная схема**

_Без параметров._


### `nexus_db_stats`

Get database statistics: memory count, entity count, relationship count, commit count, snapshot count, and DB file size. Use this for health/status reports.

**Входная схема**

_Без параметров._


### `nexus_health`

Check system health (database connectivity)

**Входная схема**

_Без параметров._


### `nexus_settings`

Get application settings

**Входная схема**

_Без параметров._


### `nexus_config_get`

Get configuration values. Pass a key to read a single value, or omit key to list ALL configuration entries. Use this to inspect app settings.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `key` | `string` | — | Config key (optional — omit to list all) |


### `nexus_config_set`

Set a configuration value (creates or updates the key). Use this to change app settings programmatically.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `key` | `string` | **да** | Config key |
| `value` | `string` | **да** | Config value |


### `nexus_timeline`

Get timeline of all entities sorted by creation date

**Входная схема**

_Без параметров._


### `nexus_ai_models`

List all available LLM models (via the opencode CLI). Pass free_only=true to list only free models. Use this to discover which models can be selected for AI features.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `free_only` | `boolean`, default: `false` | — | Only list free models |


## Документация

Импорт и поиск по документам.

### `nexus_docs_import`

Import all .md/.markdown/.txt files from a folder into the project knowledge base (RAG corpus). Idempotent: unchanged files are skipped, changed files are re-indexed, files removed from disk are pruned. Use this to make a project's documentation searchable by the AI.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `folder_path` | `string` | **да** | Absolute path of the folder to import |


### `nexus_docs_list`

List imported project documents (RAG corpus), newest first. Use this to see what documentation is already indexed before searching.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | Max results (default 100) |


### `nexus_docs_search`

Search the imported project documentation (RAG corpus) by a query. Combines keyword overlap with semantic similarity (ONNX embeddings when available). Returns matching documents with relevance scores so the AI can answer questions about the project's own docs.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | Max results (default 10) |
| `query` | `string` | **да** | Search query |


## Агенты и политики доступа

Агенты, паспорта, политики, проверка доступа.

### `nexus_agents_read`

Read the project's AGENTS.md instruction file (or another agents file by name). The content is already injected into context packages automatically, but use this to see the exact rules the AI is expected to follow.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `name` | `string` | — | Agents file name (default: AGENTS.md) |


### `nexus_agents_generate`

Generate an AGENTS.md from live Nexus data (modules, commands, knowledge base state) and store it as the active instruction file. The 'documentation skill': use this to create or refresh project instructions without writing them by hand.

**Входная схема**

_Без параметров._


### `nexus_agent_policy_add`

Firewall agent permissions — create/update the policy of WHO may see WHAT memory. Pass agent name, optional role, allowed visibility (public,private,restricted; comma-separated, empty=all), allowed layers (working,episodic,semantic,procedural,decision,strategic; empty=all), deny patterns (comma-separated; any match in title/summary/content denies the agent). Example: Claude Code sees architecture/code/decisions but never secrets or personal memory — enterprise-grade access control, not a plain ACL.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `agent` | `string` | **да** | Agent name (e.g. claude-code) |
| `allowedLayers` | `string` | — | Comma-separated layers (empty = all) |
| `allowedVisibility` | `string` | — | Comma-separated: public,private,restricted (empty = all) |
| `denyPatterns` | `string` | — | Comma-separated forbidden substrings (e.g. 'api key,password,паспорт') |
| `role` | `string` | — | Optional role label (assistant/reviewer/automation) |


### `nexus_agent_policy_list`

List all agent-level memory permission policies — who may see what memory categories, layers and visibilities, plus deny patterns.

**Входная схема**

_Без параметров._


### `nexus_agent_access_check`

Firewall access control — check whether a specific agent may see a specific memory (by memory id) BEFORE it is injected into the LLM context. Returns allow/deny with the reasons (visibility, layer or deny pattern) and the memory sensitivity level (public/project/restricted/private). Use this to enforce 'what should this agent know' — the second ring of the Memory Firewall.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `agent` | `string` | **да** | Agent name (e.g. claude-code) |
| `memoryId` | `string` | **да** | Memory id to check access for |


## Паспорта агентов

Паспорта агентов: получение, список, рендер, upsert.

### `nexus_passport_get`

Get the identity passport of an agent: role, memory scope, trust level, available skills, allowed tools and constraints. Use it to confirm who you are and what you are allowed to do before acting. Falls back to the default primary passport when the name does not exist.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `name` | `string` | **да** | Agent name, e.g. opencode-primary |


### `nexus_passport_list`

List all agent identity passports registered in the system. Each passport describes an agent's role (generalist, coder, researcher, reviewer, orchestrator, memory-keeper), memory scope, trust level and allowed skills/tools/constraints.

**Входная схема**

_Без параметров._


### `nexus_passport_render`

Render an agent's passport as a compact markdown block (identity, role, skills, tools, constraints, trust). Use it to attach the passport to a context package or an AGENTS.md file so the AI knows its own boundaries.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `name` | `string` | **да** | Agent name, e.g. opencode-primary |


### `nexus_passport_upsert`

Create or update an agent identity passport by name. Fields update in place when the passport exists, or create a fresh one. Role is one of generalist, coder, researcher, reviewer, orchestrator, memory-keeper; memory_scope is personal, project, team or global; trust_level is 1..10 (how much the agent's memories can be trusted).

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `constraints` | `array` | — | Things the agent must NOT do |
| `description` | `string` | — | What the agent does (1-2 sentences) |
| `display_name` | `string` | — | Human-readable name |
| `memory_scope` | `string` | — | personal|project|team|global |
| `name` | `string` | **да** | Agent name (unique identity) |
| `role` | `string` | — | generalist|coder|researcher|reviewer|orchestrator|memory-keeper |
| `skills` | `array` | — | Available skill names |
| `tools` | `array` | — | Allowed MCP tool names |
| `trust_level` | `number` | — | 1..10 trust in the agent's memories |


## Firewall и карантин

Проверка контента, правила, карантин.

### `nexus_firewall_check`

Preview how the Memory Firewall would treat a piece of incoming content before it is stored: returns a verdict (allow|block|quarantine), heuristic scores (toxicity, spam, prompt-injection, pii) and the reasons. Read-only — does not store anything. Use it to screen AI-generated or imported text before calling nexus_create_memory.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `content` | `string` | **да** | Memory content to screen |
| `title` | `string` | **да** | Memory title |


### `nexus_firewall_rule_add`

Add a user-defined Memory Firewall rule. When the pattern is found in incoming content, the rule overrides the heuristics: action 'block' rejects the content outright, action 'quarantine' parks it in the quarantine table for a human to approve or reject. Pattern matching is case-insensitive substring matching.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `action` | `string` | **да** | block | quarantine |
| `pattern` | `string` | **да** | Substring to match in title+content |
| `reason` | `string` | — | Optional human-readable reason |


### `nexus_firewall_rule_delete`

Delete a user-defined Memory Firewall rule by id.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Rule id to delete |


### `nexus_firewall_rules`

List all user-defined Memory Firewall rules: each rule has a pattern (matched case-insensitively in title+content), an action (block|quarantine), an enabled flag, an optional reason and its creation time.

**Входная схема**

_Без параметров._


### `nexus_quarantine_approve`

Approve a quarantined entry: creates a real memory from its content (bypassing the firewall — the human explicitly confirmed it) and marks the entry approved. The memory goes through the normal classification, conflict detection and semantic indexing pipelines.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Quarantine entry id to approve |


### `nexus_quarantine_list`

List quarantine entries — content the firewall screened but did not hard-block. Default shows pending entries that need a human decision. Pass status=pending|approved|rejected to filter, or status=all for everything.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `status` | `string` | — | pending | approved | rejected | all (default pending) |


### `nexus_quarantine_reject`

Reject a quarantined entry: the content is permanently discarded (marked rejected) and never enters the memory store.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Quarantine entry id to reject |


## Flight Recorder и контекстные цепочки

Журнал полёта, replay, why-цепочки.

### `nexus_flight_active_sessions`

List currently active flight recorder sessions — what operation runs are in progress right now (agent passes, tool batches, tasks).

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `number` | — | Max sessions (default 20) |


### `nexus_flight_log`

Manually log an operation into the flight recorder (the system's black box). Use it to record significant agent actions that are not automatically captured: category, action, summary, entity, outcome and optional details. Nothing is stored in memory — only appended to the operation journal.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `action` | `string` | **да** | Verb describing the operation, e.g. create_memory, resolve_conflict, run_cycle |
| `category` | `string` | **да** | memory | conflict | firewall | rehearsal | radar | skill | context | team | versioning | mcp | system |
| `details` | `object` | — | Optional extra details (JSON) |
| `duration_ms` | `number` | — | Optional duration of the operation in ms |
| `entity_id` | `string` | — | Optional entity id |
| `entity_type` | `string` | — | Optional entity type, e.g. MemoryRecord |
| `outcome` | `string` | — | success | error | blocked | skipped (default success) |
| `summary` | `string` | **да** | One-line human-readable description |


### `nexus_flight_recent`

List the most recent flight recorder entries (the system's operation black box). Optionally filter by category. Useful to understand what the system has been doing and why.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `category` | `string` | — | Filter: memory | conflict | firewall | rehearsal | radar | skill | context | team | versioning | mcp | system |
| `limit` | `number` | — | Max entries (default 50) |


### `nexus_flight_replay`

Replay the full operation chain of one entity from the flight recorder — every recorded step that touched the entity, chronological. Like reading the black box of a specific memory: created, quarantined, approved, updated, superseded, etc.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `entity_id` | `string` | **да** | Entity id to replay |
| `entity_type` | `string` | **да** | Entity type, e.g. MemoryRecord |


### `nexus_flight_stats`

Flight recorder summary statistics: total records, sessions, active sessions, and counts broken down by category and outcome. A health check for the operation black box.

**Входная схема**

_Без параметров._


### `nexus_context_chain_recent`

List recently recorded context chains (newest first) with their query, intent, answer confidence and 'why' breakdown. Use it to find the id of a past answer you need to explain.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | Max chains to return (default 10) |


### `nexus_context_chain_record`

Record the full context chain of an answer: the query, intent, answer confidence, memory seeds used (kind, memoryId, title, weight, tokens) and pipeline stages (stage, durationMs, note). Every model answer becomes explainable afterwards via nexus_why.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `actor` | `string` | — | Agent/actor that answered |
| `answer` | `string` | — | Final model answer text |
| `confidence` | `number` | — | Answer confidence 0..1 |
| `intent` | `string` | — | Intent label, e.g. explain_architecture |
| `query` | `string` | **да** | The user query that started the pipeline |
| `seeds_json` | `string` | — | JSON array of {kind, memoryId, title, weight, tokens} |
| `stages_json` | `string` | — | JSON array of {stage, durationMs, note} |


### `nexus_why`

Explain a past AI answer: fetch a recorded context chain by id and return 'Why did AI say this?' — a breakdown of which memory seeds and pipeline stages produced the answer, with an ASCII bar chart of context shares by category and the most influential memories. Call it whenever a user asks why a previous answer came out the way it did.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Context chain id (from nexus_context_chain_recent) |


## Rehearsal и канонические воспоминания

План повторения, цикл, консолидация.

### `nexus_rehearsal_consolidate`

Memory Rehearsal — canonical consolidation: finds records that state the same fact and collapses them into one Canonical Memory (e.g. 7 notes about JWT auth become one 'Authentication uses JWT access tokens + rotating refresh tokens'), boosting importance/confidence by repetition while keeping full provenance (derived_from). Run it in the sleep cycle — this is how Nexus' memory evolves on its own.

**Входная схема**

_Без параметров._


### `nexus_rehearsal_cycle`

Run the memory rehearsal (sleep) cycle over the whole pool: rehearses every due memory (strengthens importance/confidence, reschedules the next review with a longer interval), schedules first rehearsals for fresh memories, and decays old never-rehearsed memories so they stop competing for context space. Returns a report of what was rehearsed, scheduled and decayed. Safe to run any time.

**Входная схема**

_Без параметров._


### `nexus_rehearsal_plan`

Memory rehearsal plan: which memories are due for review right now, ordered by importance. Rehearsal is the spaced-repetition cycle that keeps important knowledge fresh and lets forgotten memories fade. Use this periodically (e.g. at session start) to see what should be reviewed next. Read-only — does not modify anything.

**Входная схема**

_Без параметров._


### `nexus_rehearse_memory`

Mark a single memory as rehearsed right now — call this after a human (or the AI) actually reviewed the memory. Strengthens the memory slightly, bumps its rehearsal counter and reschedules the next review with a longer interval.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Memory id to rehearse |


### `nexus_canonical_memories`

List the canonical memories produced by consolidation — the distilled truths of the project (one record instead of many duplicates). Pass limit (default 25).

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | How many canonical memories to return (default 25) |


## Context Lab

A/B/C-эксперименты стратегий сборки контекста.

### `nexus_context_lab_history`

Context Lab history — returns recent lab experiments (newest first): which strategies won on which queries and the measured accuracy predictions. Use it to see what Nexus learned about choosing a context strategy.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | Max experiments to return (default 10) |


### `nexus_context_lab_run`

Context Lab — runs one query through multiple context-building strategies (compact / balanced / rich), measures each: memories included, tokens, maturity of cognitive layers, average relevance, build time, and a PREDICTED ACCURACY of the answer per strategy, then recommends the best one. Use it to answer 'how much context is enough for this question?' and to tune the engine's strategy per query type.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `query` | `string` | **да** | The question to build context for |


### `nexus_context_lab_stats`

Context Lab stats — how many experiments were run and which strategy wins most often overall. Use it to check whether Nexus is converging on a preferred context strategy.

**Входная схема**

_Без параметров._


## Predictive

Предсказание следующего запроса и сущностей.

### `nexus_predictive_predict`

Predictive Context — predicts the NEXT question the user is likely to ask, based on the Markov chain of past queries (every build_context call is remembered automatically). Returns ranked predictions with confidence, the predicted intent and the entities to prewarm in the context cache. Use it to answer 'what will they ask next?' and to pre-load context so the next answer comes instantly — no other project anticipates the user's next question like this.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `query` | `string` | **да** | The current question, to predict what comes next |
| `topK` | `integer` | — | How many predictions to return (default 3) |


### `nexus_predictive_stats`

Predictive Context stats — how many queries are in the history (the larger the history, the smarter the next-question predictions).

**Входная схема**

_Без параметров._


## Knowledge Map

Карта сущности четырьмя кольцами.

### `nexus_knowledge_map`

Knowledge Navigation 2.0 — renders the AI Universe map for an entity: concentric rings (Mission / Relevant / Supporting / Historical) around a concept, built from graph neighbors, linked memories, open conflicts and superseded records. Returns an ASCII-art map that shows what is mission-critical, what supports it, and what has been superseded — navigate any project space like a star chart, nothing else does this.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `depth` | `integer` | — | Graph traversal depth (default 1) |
| `entityId` | `string` | **да** | Entity id to build the map around |


## Скиллы и Skill Genesis

Список скиллов, запуск, сканирование кандидатов.

### `nexus_skills_list`

List all registered skills (runnable commands like JS scripts) with their descriptions. Skills are the lightweight alternative to MCP tools — an agent reads the list, picks the relevant one, and runs only it instead of carrying every tool in context.

**Входная схема**

_Без параметров._


### `nexus_skills_run`

Run a registered skill by name with optional arguments. Captures stdout/stderr with a 30-second timeout. Use this to execute a project script or automation without loading the full MCP tool surface.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `args` | `array` | — | Arguments passed to the skill |
| `name` | `string` | **да** | Skill name |


### `nexus_skill_genesis_approve`

Skill Genesis — approve a candidate: creates a real runnable skill in the skills table (name + generated description; command placeholder, to be filled by the agent) and marks the proposal approved. Pass the proposal id from nexus_skill_genesis_candidates.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Proposal id to approve |


### `nexus_skill_genesis_candidates`

Skill Genesis — list the current skill candidates (status: proposed | approved | rejected | all). Use it to review what Nexus discovered as repeatable operations before approving or rejecting them.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `status` | `string` | — | Filter by status (default all) |


### `nexus_skill_genesis_reject`

Skill Genesis — reject a candidate: marks it rejected (no skill is created) so it is not proposed again. Pass the proposal id.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Proposal id to reject |


### `nexus_skill_genesis_scan`

Skill Genesis — scans the flight log for REPEATED operations (same category+action performed N+ times) and proposes turning them into skills. Returns the new proposals with generated names, descriptions and occurrence counts. Use it to answer 'what are we doing over and over that should be a skill?' — Nexus noticing its own habits, no other project does this.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | How many recent flight records to analyze (default 2000) |
| `minOccurrences` | `integer` | — | Minimum repetitions to become a candidate (default 3) |


## Код

Индексация и поиск по коду, зависимости.

### `nexus_code_import`

Index a folder of source files into the code graph. Symbols (classes, functions, structs, traits) are extracted with the built-in language parsers, and dependency edges (import / require / use / #include / mod) are recorded. Use this to let the AI answer structural questions about a codebase: what depends on what, where a symbol is defined.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `folder_path` | `string` | **да** | Absolute path of the folder to index |


### `nexus_code_list`

List indexed source files in the code graph, newest first. Use this to see what code has been indexed before searching symbols.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | Max results (default 100) |


### `nexus_code_search`

Search symbols (classes, functions, structs, traits, interfaces) by name across all indexed source files. Returns the defining file and language. Use this to locate where something is defined in the project.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `limit` | `integer` | — | Max results (default 20) |
| `query` | `string` | **да** | Symbol name or substring |


### `nexus_code_deps`

Return the dependencies of one indexed source file (by path): what it imports, requires, includes or uses, with internal/external classification. Use this to understand a file's connections in the code graph.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `path` | `string` | **да** | Path of the indexed file |


### `nexus_code_dependents`

Return the files in the code graph that depend on the given target (reverse edges, internal only). Use this to answer 'what would be affected by changing X?'

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `target` | `string` | **да** | Dependency target (module or file) |


## Радар

Снимок окружения.

### `nexus_radar_snapshot`

Proactive memory radar: scans the whole memory pool and returns what needs attention right now — unresolved conflicts, memories expiring soon, inferred memories never confirmed by a human, and important memories created or changed since the last radar scan. Use this at the start of a session (or when opening a project) to see what the user should review, instead of waiting for a query. Optionally pass markSeen=true to advance the radar checkpoint to now so the next scan only reports what changed afterwards.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `markSeen` | `boolean` | — | Advance the scan checkpoint to now after building the snapshot (default false) |


## Команда

Участники и обзор команды.

### `nexus_team_add_member`

Add a new member to the team roster. The team roster powers the trusted decision layer (who confirmed what, what went stale, what is in conflict). Role is one of admin, member, viewer (default member).

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `name` | `string` | **да** | Member name (must be unique) |
| `role` | `string` | — | admin | member | viewer (default member) |


### `nexus_team_list_members`

List all members of the team roster with their roles and active flags.

**Входная схема**

_Без параметров._


### `nexus_team_update_member`

Update a team member's role and/or active flag by id.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `active` | `boolean` | — | Whether the member is active |
| `id` | `string` | **да** | Member id |
| `role` | `string` | — | admin | member | viewer |


### `nexus_team_remove_member`

Remove a team member from the roster by id.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `id` | `string` | **да** | Member id |


### `nexus_team_overview`

The trusted decision layer of the team: who confirmed which decision, what went stale (superseded), what is in conflict, and per-member activity (authored/confirmed/updated counts). This is the answer to 'what does the team actually know and agree on' — teams cannot get this from chat history.

**Входная схема**

_Без параметров._


## Аудит

Журнал аудита событий.

### `nexus_audit_trail`

Reconstruct the full decision chain for one memory — the answer to 'why did we decide this?'. Returns the decision context (reason), the alternatives that were considered and rejected, who confirmed the decision and when, which memory it superseded and which replaced it, and the full version history (who changed what, with diff reasons). Use this for compliance questions like 'why did we choose PostgreSQL in March?' — prove the team knew and why it decided so.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `memoryId` | `string` | **да** | Memory id to reconstruct the audit trail for |


### `nexus_audit_add_event`

Append a raw event to a memory's decision journal: Created, Confirmed, Superseded or Note. Every auditable action on a memory gets one row so the full chain 'why did we decide this' can be reconstructed. actor is who performed the action (member / user / system). For Superseded events pass relatedMemoryId pointing at the memory that replaced it.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `actor` | `string` | **да** | Who performed the action |
| `detail` | `string` | — | Optional free text |
| `eventType` | `string` | **да** | Created | Confirmed | Superseded | Note |
| `memoryId` | `string` | **да** | Memory id the event belongs to |
| `relatedMemoryId` | `string` | — | For Superseded: the memory that replaced this one |


### `nexus_audit_alternative`

Record that an alternative was considered for a decision (and rejected). Appends an Alternative event with { title, reason } to the memory's decision journal, so the compliance chain shows which options were weighed and why they lost.

**Входная схема**

| Параметр | Тип | Обязателен | Описание |
|---|---|---|---|
| `actor` | `string` | **да** | Who considered it |
| `memoryId` | `string` | **да** | Memory id the decision belongs to |
| `reason` | `string` | **да** | Why it was not chosen (e.g. license costs) |
| `title` | `string` | **да** | The alternative that was considered (e.g. MySQL) |


