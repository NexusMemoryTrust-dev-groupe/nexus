import { invoke } from '@tauri-apps/api/core';
import type { Memory, GraphNode, GraphEdge } from '../types';

// ── Command result types ──
export interface CommandResult {
  command: string;
  success: boolean;
  data: unknown;
  error?: string;
}

// ── Parsed command ──
interface ParsedCommand {
  name: string;
  args: string[];
}

/**
 * Parse a slash command string into name + args.
 * Examples:
 *   "/memories" → { name: "memories", args: [] }
 *   "/memory abc-123" → { name: "memory", args: ["abc-123"] }
 *   "/create-entity Project My Project" → { name: "create-entity", args: ["Project", "My", "Project"] }
 */
function parseCommand(input: string): ParsedCommand | null {
  const trimmed = input.trim();
  if (!trimmed.startsWith('/')) return null;

  const parts = trimmed.slice(1).split(/\s+/);
  if (parts.length === 0 || !parts[0]) return null;

  return {
    name: parts[0].toLowerCase(),
    args: parts.slice(1),
  };
}

/**
 * Execute a single slash command via Tauri IPC.
 * Returns formatted result for display.
 */
export async function executeCommand(commandStr: string): Promise<CommandResult> {
  const parsed = parseCommand(commandStr);
  if (!parsed) {
    return {
      command: commandStr,
      success: false,
      data: null,
      error: 'Invalid command format. Commands start with /',
    };
  }

  try {
    switch (parsed.name) {
      // ── Memory commands ──
      case 'memories': {
        const memories = await invoke<Memory[]>('get_memories');
        return {
          command: parsed.name,
          success: true,
          data: memories,
        };
      }

      case 'memory': {
        const id = parsed.args[0];
        if (!id) return { command: parsed.name, success: false, data: null, error: 'Missing memory ID. Usage: /memory <id>' };
        const memory = await invoke<Memory | null>('get_memory', { id });
        if (!memory) return { command: parsed.name, success: false, data: null, error: `Memory '${id}' not found` };
        return { command: parsed.name, success: true, data: memory };
      }

      case 'create-memory': {
        const title = parsed.args.join(' ') || 'New Memory';
        const memory = await invoke<Memory>('create_memory', {
          title,
          content: `Created via AI Co-Pilot: ${title}`,
          author: 'ai-copilot',
        });
        return { command: parsed.name, success: true, data: memory };
      }

      case 'search': {
        const query = parsed.args.join(' ');
        if (!query) return { command: parsed.name, success: false, data: null, error: 'Missing search query. Usage: /search <query>' };
        const results = await invoke<Memory[]>('search_memories', { query });
        return { command: parsed.name, success: true, data: results };
      }

      // ── Graph commands ──
      case 'graph': {
        const graph = await invoke<{ nodes: GraphNode[]; edges: GraphEdge[] }>('get_graph');
        return { command: parsed.name, success: true, data: graph };
      }

      case 'entity': {
        const id = parsed.args[0];
        if (!id) return { command: parsed.name, success: false, data: null, error: 'Missing entity ID. Usage: /entity <id>' };
        const entity = await invoke<GraphNode | null>('get_entity', { id });
        if (!entity) return { command: parsed.name, success: false, data: null, error: `Entity '${id}' not found` };
        return { command: parsed.name, success: true, data: entity };
      }

      case 'create-entity': {
        if (parsed.args.length < 2) {
          return { command: parsed.name, success: false, data: null, error: 'Usage: /create-entity <type> <title>' };
        }
        const entityType = parsed.args[0];
        const title = parsed.args.slice(1).join(' ');
        const entity = await invoke<GraphNode>('create_entity', {
          entityType,
          title,
          description: `Created via AI Co-Pilot`,
        });
        return { command: parsed.name, success: true, data: entity };
      }

      case 'update-memory': {
        if (parsed.args.length < 2) {
          return { command: parsed.name, success: false, data: null, error: 'Usage: /update-memory <id> <new_content>' };
        }
        const memId = parsed.args[0];
        const memContent = parsed.args.slice(1).join(' ');
        const updatedMemory = await invoke<Memory>('update_memory', {
          id: memId,
          content: memContent,
        });
        return { command: parsed.name, success: true, data: updatedMemory };
      }

      case 'delete-memory': {
        const delMemId = parsed.args[0];
        if (!delMemId) return { command: parsed.name, success: false, data: null, error: 'Missing memory ID. Usage: /delete-memory <id>' };
        await invoke('delete_memory', { id: delMemId });
        return { command: parsed.name, success: true, data: { deleted: true, id: delMemId } };
      }

      case 'update-entity': {
        if (parsed.args.length < 2) {
          return { command: parsed.name, success: false, data: null, error: 'Usage: /update-entity <id> <new_title>' };
        }
        const entId = parsed.args[0];
        const entTitle = parsed.args.slice(1).join(' ');
        const updatedEntity = await invoke<GraphNode>('update_entity', {
          id: entId,
          title: entTitle,
        });
        return { command: parsed.name, success: true, data: updatedEntity };
      }

      case 'delete-entity': {
        const delEntId = parsed.args[0];
        if (!delEntId) return { command: parsed.name, success: false, data: null, error: 'Missing entity ID. Usage: /delete-entity <id>' };
        await invoke('delete_entity', { id: delEntId });
        return { command: parsed.name, success: true, data: { deleted: true, id: delEntId } };
      }

      case 'link': {
        if (parsed.args.length < 2) {
          return { command: parsed.name, success: false, data: null, error: 'Usage: /link <project_id> <entity_id> [type] [weight]' };
        }
        const projectId = parsed.args[0];
        const entityId = parsed.args[1];
        const relType = parsed.args[2] || undefined;
        const weight = parsed.args[3] ? parseFloat(parsed.args[3]) : undefined;
        const linkResult = await invoke<GraphEdge>('link_entity_to_project', {
          projectId,
          entityId,
          relationshipType: relType,
          weight,
        });
        return { command: parsed.name, success: true, data: linkResult };
      }

      case 'unlink': {
        const relId = parsed.args[0];
        if (!relId) return { command: parsed.name, success: false, data: null, error: 'Missing relationship ID. Usage: /unlink <id>' };
        await invoke('delete_relationship', { relationshipId: relId });
        return { command: parsed.name, success: true, data: { deleted: true, id: relId } };
      }

      case 'settings': {
        const allConfig = await invoke<Array<{ key: string; value: string }>>('get_all_config');
        const theme = allConfig.find(e => e.key === 'app.theme')?.value || 'dark';
        const language = allConfig.find(e => e.key === 'app.language')?.value || 'en';
        return {
          command: parsed.name,
          success: true,
          data: { theme, language },
        };
      }

      case 'timeline': {
        const graph = await invoke<{ nodes: GraphNode[]; edges: GraphEdge[] }>('get_graph');
        // Sort nodes by creation date descending
        const sorted = [...graph.nodes].sort((a, b) =>
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        );
        return { command: parsed.name, success: true, data: sorted };
      }

      // ── Context commands ──
      case 'context': {
        const query = parsed.args.join(' ') || 'general';
        const context = await invoke<{
          token_count: number;
          entities: GraphNode[];
          memory_records: Memory[];
          relationships: GraphEdge[];
        }>('build_context', { query });
        return { command: parsed.name, success: true, data: context };
      }

      // ── System commands ──
      case 'stats': {
        const stats = await invoke<{
          memory_count: number;
          entity_count: number;
          relationship_count: number;
          commit_count: number;
          snapshot_count: number;
          db_size_bytes: number;
        }>('get_db_stats');
        return { command: parsed.name, success: true, data: stats };
      }

      case 'health': {
        const status = await invoke<string>('ai_health_check');
        return { command: parsed.name, success: true, data: status };
      }

      default:
        return {
          command: parsed.name,
          success: false,
          data: null,
          error: `Unknown command: /${parsed.name}. Type /help for available commands.`,
        };
    }
  } catch (err) {
    return {
      command: parsed.name,
      success: false,
      data: null,
      error: String(err),
    };
  }
}

/**
 * Format a command result for display in the chat.
 */
export function formatCommandResult(result: CommandResult): string {
  const { command, success, data, error } = result;

  if (!success) {
    return `**/${command}** ❌ Error: ${error}`;
  }

  switch (command) {
    case 'memories': {
      const memories = data as Memory[];
      if (memories.length === 0) return `**/${command}** ✅ No memories found.`;
      const rows = memories.map((m, i) =>
        `| ${i + 1} | ${m.title} | ${m.layer} | ${m.importanceScore.toFixed(1)} |`
      ).join('\n');
      return `**/${command}** ✅ Found ${memories.length} memories\n\n| # | Title | Layer | Importance |\n|---|-------|-------|------------|\n${rows}`;
    }

    case 'memory': {
      const m = data as Memory;
      return `**/${command}** ✅\n\n- **Title**: ${m.title}\n- **Layer**: ${m.layer}\n- **Importance**: ${m.importanceScore}\n- **Confidence**: ${m.confidenceScore}\n- **Created**: ${m.createdAt}\n- **Content**: ${m.content}`;
    }

    case 'create-memory': {
      const m = data as Memory;
      return `**/${command}** ✅ Memory created\n\n- **ID**: ${m.id}\n- **Title**: ${m.title}\n- **Layer**: ${m.layer}`;
    }

    case 'search': {
      const memories = data as Memory[];
      if (memories.length === 0) return `**/${command}** ✅ No results found.`;
      const list = memories.map((m, i) => `${i + 1}. **${m.title}** (${m.layer}, importance: ${m.importanceScore})`).join('\n');
      return `**/${command}** ✅ Found ${memories.length} results\n\n${list}`;
    }

    case 'graph': {
      const graph = data as { nodes: GraphNode[]; edges: GraphEdge[] };
      return `**/${command}** ✅\n\n- **Nodes**: ${graph.nodes.length}\n- **Edges**: ${graph.edges.length}`;
    }

    case 'entity': {
      const e = data as GraphNode;
      return `**/${command}** ✅\n\n- **Title**: ${e.title}\n- **Type**: ${e.entityType}\n- **Status**: ${e.status}\n- **Description**: ${e.description}`;
    }

    case 'create-entity': {
      const e = data as GraphNode;
      return `**/${command}** ✅ Entity created\n\n- **ID**: ${e.id}\n- **Title**: ${e.title}\n- **Type**: ${e.entityType}`;
    }

    case 'update-memory': {
      const m = data as Memory;
      return `**/${command}** ✅ Memory updated\n\n- **Title**: ${m.title}\n- **ID**: ${m.id}`;
    }

    case 'delete-memory': {
      const d = data as { deleted: boolean; id: string };
      return `**/${command}** ✅ Memory deleted\n\n- **ID**: ${d.id}`;
    }

    case 'update-entity': {
      const e = data as GraphNode;
      return `**/${command}** ✅ Entity updated\n\n- **Title**: ${e.title}\n- **ID**: ${e.id}`;
    }

    case 'delete-entity': {
      const d = data as { deleted: boolean; id: string };
      return `**/${command}** ✅ Entity deleted\n\n- **ID**: ${d.id}`;
    }

    case 'link': {
      const r = data as GraphEdge;
      return `**/${command}** ✅ Entities linked\n\n- **Type**: ${r.relationshipType}\n- **Weight**: ${r.weight}`;
    }

    case 'unlink': {
      const d = data as { deleted: boolean; id: string };
      return `**/${command}** ✅ Relationship deleted\n\n- **ID**: ${d.id}`;
    }

    case 'settings': {
      const s = data as { theme: string; language: string };
      return `**/${command}** ✅ Settings\n\n- **Theme**: ${s.theme}\n- **Language**: ${s.language}`;
    }

    case 'timeline': {
      const nodes = data as GraphNode[];
      if (nodes.length === 0) return `**/${command}** ✅ No events found.`;
      const list = nodes.slice(0, 10).map((n, i) =>
        `${i + 1}. **${n.title}** (${n.entityType}, ${new Date(n.createdAt).toLocaleDateString()})`
      ).join('\n');
      return `**/${command}** ✅ Timeline: ${nodes.length} events\n\n${list}`;
    }

    case 'context': {
      const ctx = data as { token_count: number; entities: GraphNode[]; memory_records: Memory[]; relationships: GraphEdge[] };
      return `**/${command}** ✅ Context built\n\n- **Tokens**: ${ctx.token_count}\n- **Entities**: ${ctx.entities.length}\n- **Memories**: ${ctx.memory_records.length}\n- **Relationships**: ${ctx.relationships.length}`;
    }

    case 'stats': {
      const s = data as { memory_count: number; entity_count: number; relationship_count: number; commit_count: number; snapshot_count: number; db_size_bytes: number };
      const sizeKB = (s.db_size_bytes / 1024).toFixed(1);
      return `**/${command}** ✅\n\n- **Memories**: ${s.memory_count}\n- **Entities**: ${s.entity_count}\n- **Relationships**: ${s.relationship_count}\n- **Commits**: ${s.commit_count}\n- **Snapshots**: ${s.snapshot_count}\n- **DB Size**: ${sizeKB} KB`;
    }

    case 'health': {
      return `**/${command}** ✅ ${data}`;
    }

    default:
      return `**/${command}** ✅ Done`;
  }
}

/**
 * Check if a message contains a slash command and execute it.
 * Returns formatted result string if command found, null otherwise.
 */
export async function tryExecuteCommand(message: string): Promise<string | null> {
  const trimmed = message.trim();
  if (!trimmed.startsWith('/')) return null;

  const result = await executeCommand(trimmed);
  return formatCommandResult(result);
}
