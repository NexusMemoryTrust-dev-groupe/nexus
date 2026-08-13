// Plan 7.7: generates docs/mcp/ from docs/mcp/tools.json (dumped from the
// running server's tool_definitions() by the ignored test
// `dump_tool_schemas_for_docs`). Single source of truth stays in Rust; these
// markdown files are a rendered snapshot, regenerable on demand:
//
//   node scripts/generate-mcp-docs.mjs
//
// Output:
//   docs/mcp/README.md    — index: categories, counts, how to connect
//   docs/mcp/reference.md — full reference: every tool with its input schema

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const toolsJson = join(root, 'docs', 'mcp', 'tools.json');

const { tools } = JSON.parse(readFileSync(toolsJson, 'utf8'));
if (!Array.isArray(tools) || tools.length === 0) {
  console.error('docs/mcp/tools.json is empty or missing.');
  console.error('Regenerate it with:');
  console.error('  $env:NEXUS_DUMP_TOOLS = "docs\\mcp\\tools.json"');
  console.error('  cargo test --lib dump_tool_schemas_for_docs -- --ignored');
  process.exit(1);
}

// ── Category assignment ──────────────────────────────────────────────────────
// Each entry lists the tool names it owns. `null` category means the tool is
// deliberately uncategorized (unknown prefix) and is listed under "Прочее".
const CATEGORIES = [
  { name: 'Память', desc: 'CRUD и поиск записей памяти.', tools: [
    'nexus_list_memories', 'nexus_get_memory', 'nexus_create_memory',
    'nexus_update_memory', 'nexus_delete_memory', 'nexus_search_memories',
    'nexus_search_semantic', 'nexus_get_recent_memories',
    'nexus_get_important_memories', 'nexus_store_fingerprint',
    'nexus_memory_score',
  ] },
  { name: 'Когнитивные слои', desc: 'Шесть когнитивных слоёв и их провенанс.', tools: [
    'nexus_layers_list', 'nexus_layer_stats', 'nexus_layer_set',
    'nexus_layer_reclassify', 'nexus_layer_history',
  ] },
  { name: 'Жизненный цикл памяти', desc: 'Подтверждение, устаревание, конфликты, обратная связь.', tools: [
    'nexus_memory_set_state', 'nexus_memory_confirm', 'nexus_memory_feedback',
    'nexus_memory_supersede', 'nexus_lifecycle_overview',
  ] },
  { name: 'Конфликты', desc: 'Обнаружение, разбор и разрешение противоречий.', tools: [
    'nexus_conflict_check', 'nexus_conflict_details', 'nexus_conflict_list',
    'nexus_conflict_resolve', 'nexus_conflict_truth',
  ] },
  { name: 'Сущности — дедупликация', desc: 'Поиск и слияние дубликатов графа.', tools: [
    'nexus_find_duplicates', 'nexus_merge_entities',
  ] },
  { name: 'Граф знаний', desc: 'Сущности и связи.', tools: [
    'nexus_graph_stats', 'nexus_get_entity', 'nexus_create_entity',
    'nexus_update_entity', 'nexus_delete_entity', 'nexus_link_entities',
    'nexus_unlink_entities', 'nexus_list_graph_entities',
    'nexus_entity_metadata', 'nexus_link_project_entity',
  ] },
  { name: 'Контекст и поиск', desc: 'Сборка пакета контекста и разбор текста.', tools: [
    'nexus_build_context', 'nexus_build_context_for_entity',
    'nexus_search_context', 'nexus_analyze_text', 'nexus_parse_markdown',
  ] },
  { name: 'Связи память ↔ сущность', desc: 'Привязка записей к сущностям.', tools: [
    'nexus_link_memory_entity', 'nexus_unlink_memory_entity',
    'nexus_get_memory_links', 'nexus_get_entity_memory_links',
  ] },
  { name: 'Проекты', desc: 'Рабочие проекты и их содержимое.', tools: [
    'nexus_projects', 'nexus_project_entities', 'nexus_project_memories',
  ] },
  { name: 'Рабочая область', desc: 'Пространство файлов проекта.', tools: [
    'nexus_add_to_workspace', 'nexus_get_workspace', 'nexus_sync_workspace',
    'nexus_workspace_check_stale', 'nexus_workspace_rename',
    'nexus_workspace_move', 'nexus_workspace_delete', 'nexus_workspace_remove',
  ] },
  { name: 'Файлы (песочница)', desc: 'Файловые операции внутри whitelist-корней.', tools: [
    'nexus_index_file', 'nexus_index_folder', 'nexus_scan_folder',
    'nexus_read_file_content', 'nexus_read_file', 'nexus_create_file',
    'nexus_write_file', 'nexus_create_folder', 'nexus_delete_file',
    'nexus_delete_folder', 'nexus_rename_file', 'nexus_move_file',
    'nexus_create_workspace_file',
  ] },
  { name: 'Экономия и продуктовые метрики', desc: 'Токены, сэкономленные средства, метрики ценности.', tools: [
    'nexus_savings_stats', 'nexus_savings_report', 'nexus_savings_per_model',
    'nexus_savings_record', 'nexus_product_metrics',
  ] },
  { name: 'Система и диагностика', desc: 'Здоровье, статистика, настройки, конфигурация.', tools: [
    'nexus_copilot_command', 'nexus_stats', 'nexus_db_stats', 'nexus_health',
    'nexus_settings', 'nexus_config_get', 'nexus_config_set',
    'nexus_timeline', 'nexus_ai_models',
  ] },
  { name: 'Документация', desc: 'Импорт и поиск по документам.', tools: [
    'nexus_docs_import', 'nexus_docs_list', 'nexus_docs_search',
  ] },
  { name: 'Агенты и политики доступа', desc: 'Агенты, паспорта, политики, проверка доступа.', tools: [
    'nexus_agents_read', 'nexus_agents_generate', 'nexus_agent_policy_add',
    'nexus_agent_policy_list', 'nexus_agent_access_check',
  ] },
  { name: 'Паспорта агентов', desc: 'Паспорта агентов: получение, список, рендер, upsert.', tools: [
    'nexus_passport_get', 'nexus_passport_list', 'nexus_passport_render',
    'nexus_passport_upsert',
  ] },
  { name: 'Firewall и карантин', desc: 'Проверка контента, правила, карантин.', tools: [
    'nexus_firewall_check', 'nexus_firewall_rule_add',
    'nexus_firewall_rule_delete', 'nexus_firewall_rules',
    'nexus_quarantine_approve', 'nexus_quarantine_list',
    'nexus_quarantine_reject',
  ] },
  { name: 'Flight Recorder и контекстные цепочки', desc: 'Журнал полёта, replay, why-цепочки.', tools: [
    'nexus_flight_active_sessions', 'nexus_flight_log', 'nexus_flight_recent',
    'nexus_flight_replay', 'nexus_flight_stats', 'nexus_context_chain_recent',
    'nexus_context_chain_record', 'nexus_why',
  ] },
  { name: 'Rehearsal и канонические воспоминания', desc: 'План повторения, цикл, консолидация.', tools: [
    'nexus_rehearsal_consolidate', 'nexus_rehearsal_cycle',
    'nexus_rehearsal_plan', 'nexus_rehearse_memory',
    'nexus_canonical_memories',
  ] },
  { name: 'Context Lab', desc: 'A/B/C-эксперименты стратегий сборки контекста.', tools: [
    'nexus_context_lab_history', 'nexus_context_lab_run',
    'nexus_context_lab_stats',
  ] },
  { name: 'Predictive', desc: 'Предсказание следующего запроса и сущностей.', tools: [
    'nexus_predictive_predict', 'nexus_predictive_stats',
  ] },
  { name: 'Knowledge Map', desc: 'Карта сущности четырьмя кольцами.', tools: [
    'nexus_knowledge_map',
  ] },
  { name: 'Скиллы и Skill Genesis', desc: 'Список скиллов, запуск, сканирование кандидатов.', tools: [
    'nexus_skills_list', 'nexus_skills_run', 'nexus_skill_genesis_approve',
    'nexus_skill_genesis_candidates', 'nexus_skill_genesis_reject',
    'nexus_skill_genesis_scan',
  ] },
  { name: 'Код', desc: 'Индексация и поиск по коду, зависимости.', tools: [
    'nexus_code_import', 'nexus_code_list', 'nexus_code_search',
    'nexus_code_deps', 'nexus_code_dependents',
  ] },
  { name: 'Радар', desc: 'Снимок окружения.', tools: ['nexus_radar_snapshot'] },
  { name: 'Команда', desc: 'Участники и обзор команды.', tools: [
    'nexus_team_add_member', 'nexus_team_list_members',
    'nexus_team_update_member', 'nexus_team_remove_member',
    'nexus_team_overview',
  ] },
  { name: 'Аудит', desc: 'Журнал аудита событий.', tools: [
    'nexus_audit_trail', 'nexus_audit_add_event', 'nexus_audit_alternative',
  ] },
];

const byName = new Map(tools.map((t) => [t.name, t]));

// Validate: every dumped tool must be categorized.
const categorized = new Set(CATEGORIES.flatMap((c) => c.tools));
const missing = tools.filter((t) => !categorized.has(t.name));
if (missing.length > 0) {
  console.error(`Tools.json has ${missing.length} tools not covered by CATEGORIES:`);
  for (const m of missing) console.error(`  ${m.name}`);
  process.exit(1);
}

const assigned = new Set();
for (const c of CATEGORIES) {
  for (const name of c.tools) {
    if (assigned.has(name)) {
      console.error(`Tool ${name} assigned to two categories`);
      process.exit(1);
    }
    assigned.add(name);
  }
}

// ── Rendering helpers ───────────────────────────────────────────────────────
function schemaTable(schema) {
  const props = schema?.properties ?? {};
  const required = new Set(schema?.required ?? []);
  const entries = Object.entries(props);
  if (entries.length === 0) return '_Без параметров._\n';
  const rows = entries.map(([key, p]) => {
    const type = p.type ?? 'any';
    const desc = (p.description ?? '').replace(/\n/g, ' ');
    const req = required.has(key) ? '**да**' : '—';
    const def = p.default !== undefined ? `, default: \`${JSON.stringify(p.default)}\`` : '';
    return `| \`${key}\` | \`${type}\`${def} | ${req} | ${desc} |`;
  });
  return [
    '| Параметр | Тип | Обязателен | Описание |',
    '|---|---|---|---|',
    ...rows,
    '',
  ].join('\n');
}

function renderTool(t) {
  const deprecated = t.deprecated ? ' > ⚠️ **deprecated**' : '';
  return [
    `### \`${t.name}\`${deprecated}`,
    '',
    t.description,
    '',
    `**Входная схема**`,
    '',
    schemaTable(t.inputSchema),
    '',
  ].join('\n');
}

// ── docs/mcp/README.md ──────────────────────────────────────────────────────
const total = tools.length;
const perCat = CATEGORIES.map((c) => {
  const count = c.tools.length;
  const links = c.tools.map((n) => '`' + n + '`').join(' · ');
  return `### ${c.name} — ${count}\n\n${c.desc}\n\n${links}\n`;
}).join('\n');

const FENCE = '```';
const readme = [
  '# Nexus MCP API',
  '',
  `Model Context Protocol сервер Nexus отдаёт **${total} инструментов** любому`,
  'совместимому ИИ (OpenCode, Claude Desktop, Cursor, Continue, собственные',
  'агенты). Этот каталог — сгенерированный снимок схем инструментов.',
  '',
  '- **Полный справочник:** [reference.md](reference.md)',
  '- **Машинный дамп:** [tools.json](tools.json) (источник для генерации)',
  '',
  '## Подключение',
  '',
  FENCE,
  '"mcp": {',
  '  "nexus": {',
  '    "type": "local",',
  '    "command": ["C:\\\\Program Files\\\\Nexus\\\\Nexus.exe", "--mcp"],',
  '    "enabled": true',
  '  }',
  '}',
  FENCE,
  '',
  'См. раздел **MCP-сервер** в [README.md](../../README.md) — автоматическая',
  'регистрация в один клик, ручные конфиги для всех клиентов, пример вызова.',
  '',
  '## Категории инструментов',
  '',
  perCat,
  '',
  '## Регенерация',
  '',
  'Инструменты объявлены в Rust (`src-tauri/src/ai/mcp_server.rs`,',
  '`tool_definitions()`). Чтобы обновить этот каталог после изменения схем:',
  '',
  FENCE + 'powershell',
  '$env:NEXUS_DUMP_TOOLS = "docs\\mcp\\tools.json"',
  'cargo test --lib dump_tool_schemas_for_docs -- --ignored',
  'node scripts/generate-mcp-docs.mjs',
  FENCE,
  '',
  `> Сгенерировано: ${new Date().toISOString()}. ${total} инструментов, без`,
  '> deprecated. JSON и markdown — производные; правки вносятся в Rust-схемы.',
  '',
].join('\n');

// ── docs/mcp/reference.md ───────────────────────────────────────────────────
const sections = CATEGORIES.map((c) => {
  const body = c.tools
    .map((name) => byName.get(name))
    .filter(Boolean)
    .map(renderTool)
    .join('\n');
  return `## ${c.name}\n\n${c.desc}\n\n${body}`;
}).join('\n');

const reference = [
  '# Справочник MCP-инструментов Nexus',
  '',
  `Все **${total}** инструментов с входными схемами. Сгенерировано из`,
  '`tool_definitions()` — того же источника, что отвечает сервер на',
  '`tools/list`.',
  '',
  sections,
  '',
].join('\n');

// ── Write ───────────────────────────────────────────────────────────────────
mkdirSync(join(root, 'docs', 'mcp'), { recursive: true });
writeFileSync(join(root, 'docs', 'mcp', 'README.md'), readme, 'utf8');
writeFileSync(join(root, 'docs', 'mcp', 'reference.md'), reference, 'utf8');

const counts = CATEGORIES.map((c) => `${c.name}=${c.tools.length}`).join(', ');
console.log(`Wrote docs/mcp/README.md and docs/mcp/reference.md`);
console.log(`Total: ${total} tools. Categories: ${counts}`);
