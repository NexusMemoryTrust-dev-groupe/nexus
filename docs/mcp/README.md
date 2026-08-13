# Nexus MCP API

Model Context Protocol сервер Nexus отдаёт **143 инструментов** любому
совместимому ИИ (OpenCode, Claude Desktop, Cursor, Continue, собственные
агенты). Этот каталог — сгенерированный снимок схем инструментов.

- **Полный справочник:** [reference.md](reference.md)
- **Машинный дамп:** [tools.json](tools.json) (источник для генерации)

## Подключение

```
"mcp": {
  "nexus": {
    "type": "local",
    "command": ["C:\\Program Files\\Nexus\\Nexus.exe", "--mcp"],
    "enabled": true
  }
}
```

См. раздел **MCP-сервер** в [README.md](../../README.md) — автоматическая
регистрация в один клик, ручные конфиги для всех клиентов, пример вызова.

## Категории инструментов

### Память — 11

CRUD и поиск записей памяти.

`nexus_list_memories` · `nexus_get_memory` · `nexus_create_memory` · `nexus_update_memory` · `nexus_delete_memory` · `nexus_search_memories` · `nexus_search_semantic` · `nexus_get_recent_memories` · `nexus_get_important_memories` · `nexus_store_fingerprint` · `nexus_memory_score`

### Когнитивные слои — 5

Шесть когнитивных слоёв и их провенанс.

`nexus_layers_list` · `nexus_layer_stats` · `nexus_layer_set` · `nexus_layer_reclassify` · `nexus_layer_history`

### Жизненный цикл памяти — 5

Подтверждение, устаревание, конфликты, обратная связь.

`nexus_memory_set_state` · `nexus_memory_confirm` · `nexus_memory_feedback` · `nexus_memory_supersede` · `nexus_lifecycle_overview`

### Конфликты — 5

Обнаружение, разбор и разрешение противоречий.

`nexus_conflict_check` · `nexus_conflict_details` · `nexus_conflict_list` · `nexus_conflict_resolve` · `nexus_conflict_truth`

### Сущности — дедупликация — 2

Поиск и слияние дубликатов графа.

`nexus_find_duplicates` · `nexus_merge_entities`

### Граф знаний — 10

Сущности и связи.

`nexus_graph_stats` · `nexus_get_entity` · `nexus_create_entity` · `nexus_update_entity` · `nexus_delete_entity` · `nexus_link_entities` · `nexus_unlink_entities` · `nexus_list_graph_entities` · `nexus_entity_metadata` · `nexus_link_project_entity`

### Контекст и поиск — 5

Сборка пакета контекста и разбор текста.

`nexus_build_context` · `nexus_build_context_for_entity` · `nexus_search_context` · `nexus_analyze_text` · `nexus_parse_markdown`

### Связи память ↔ сущность — 4

Привязка записей к сущностям.

`nexus_link_memory_entity` · `nexus_unlink_memory_entity` · `nexus_get_memory_links` · `nexus_get_entity_memory_links`

### Проекты — 3

Рабочие проекты и их содержимое.

`nexus_projects` · `nexus_project_entities` · `nexus_project_memories`

### Рабочая область — 8

Пространство файлов проекта.

`nexus_add_to_workspace` · `nexus_get_workspace` · `nexus_sync_workspace` · `nexus_workspace_check_stale` · `nexus_workspace_rename` · `nexus_workspace_move` · `nexus_workspace_delete` · `nexus_workspace_remove`

### Файлы (песочница) — 13

Файловые операции внутри whitelist-корней.

`nexus_index_file` · `nexus_index_folder` · `nexus_scan_folder` · `nexus_read_file_content` · `nexus_read_file` · `nexus_create_file` · `nexus_write_file` · `nexus_create_folder` · `nexus_delete_file` · `nexus_delete_folder` · `nexus_rename_file` · `nexus_move_file` · `nexus_create_workspace_file`

### Экономия и продуктовые метрики — 5

Токены, сэкономленные средства, метрики ценности.

`nexus_savings_stats` · `nexus_savings_report` · `nexus_savings_per_model` · `nexus_savings_record` · `nexus_product_metrics`

### Система и диагностика — 9

Здоровье, статистика, настройки, конфигурация.

`nexus_copilot_command` · `nexus_stats` · `nexus_db_stats` · `nexus_health` · `nexus_settings` · `nexus_config_get` · `nexus_config_set` · `nexus_timeline` · `nexus_ai_models`

### Документация — 3

Импорт и поиск по документам.

`nexus_docs_import` · `nexus_docs_list` · `nexus_docs_search`

### Агенты и политики доступа — 5

Агенты, паспорта, политики, проверка доступа.

`nexus_agents_read` · `nexus_agents_generate` · `nexus_agent_policy_add` · `nexus_agent_policy_list` · `nexus_agent_access_check`

### Паспорта агентов — 4

Паспорта агентов: получение, список, рендер, upsert.

`nexus_passport_get` · `nexus_passport_list` · `nexus_passport_render` · `nexus_passport_upsert`

### Firewall и карантин — 7

Проверка контента, правила, карантин.

`nexus_firewall_check` · `nexus_firewall_rule_add` · `nexus_firewall_rule_delete` · `nexus_firewall_rules` · `nexus_quarantine_approve` · `nexus_quarantine_list` · `nexus_quarantine_reject`

### Flight Recorder и контекстные цепочки — 8

Журнал полёта, replay, why-цепочки.

`nexus_flight_active_sessions` · `nexus_flight_log` · `nexus_flight_recent` · `nexus_flight_replay` · `nexus_flight_stats` · `nexus_context_chain_recent` · `nexus_context_chain_record` · `nexus_why`

### Rehearsal и канонические воспоминания — 5

План повторения, цикл, консолидация.

`nexus_rehearsal_consolidate` · `nexus_rehearsal_cycle` · `nexus_rehearsal_plan` · `nexus_rehearse_memory` · `nexus_canonical_memories`

### Context Lab — 3

A/B/C-эксперименты стратегий сборки контекста.

`nexus_context_lab_history` · `nexus_context_lab_run` · `nexus_context_lab_stats`

### Predictive — 2

Предсказание следующего запроса и сущностей.

`nexus_predictive_predict` · `nexus_predictive_stats`

### Knowledge Map — 1

Карта сущности четырьмя кольцами.

`nexus_knowledge_map`

### Скиллы и Skill Genesis — 6

Список скиллов, запуск, сканирование кандидатов.

`nexus_skills_list` · `nexus_skills_run` · `nexus_skill_genesis_approve` · `nexus_skill_genesis_candidates` · `nexus_skill_genesis_reject` · `nexus_skill_genesis_scan`

### Код — 5

Индексация и поиск по коду, зависимости.

`nexus_code_import` · `nexus_code_list` · `nexus_code_search` · `nexus_code_deps` · `nexus_code_dependents`

### Радар — 1

Снимок окружения.

`nexus_radar_snapshot`

### Команда — 5

Участники и обзор команды.

`nexus_team_add_member` · `nexus_team_list_members` · `nexus_team_update_member` · `nexus_team_remove_member` · `nexus_team_overview`

### Аудит — 3

Журнал аудита событий.

`nexus_audit_trail` · `nexus_audit_add_event` · `nexus_audit_alternative`


## Регенерация

Инструменты объявлены в Rust (`src-tauri/src/ai/mcp_server.rs`,
`tool_definitions()`). Чтобы обновить этот каталог после изменения схем:

```powershell
$env:NEXUS_DUMP_TOOLS = "docs\mcp\tools.json"
cargo test --lib dump_tool_schemas_for_docs -- --ignored
node scripts/generate-mcp-docs.mjs
```

> Сгенерировано: 2026-08-11T19:45:52.242Z. 143 инструментов, без
> deprecated. JSON и markdown — производные; правки вносятся в Rust-схемы.
