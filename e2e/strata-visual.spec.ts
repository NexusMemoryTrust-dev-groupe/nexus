import { test, expect } from '@playwright/test';

const now = Date.now();
const hours = (n: number) => new Date(now - n * 3_600_000).toISOString();
const days = (n: number, hour = 11) => {
  const d = new Date(now - n * 86_400_000);
  d.setHours(hour, (n * 13) % 60, 0, 0);
  return d.toISOString();
};

const memories = [
  ['m1', 'Context assembly should explain itself', 'The package pipeline needs to show what was gathered, ranked, cut, and exported.', 'Decision', .95, .94, hours(2)],
  ['m2', 'Nexus page redesign direction', 'Keep the carbon palette, but change the information architecture completely.', 'Wisdom', .91, .89, hours(5)],
  ['m3', 'Authentication research notes', 'A verified summary of session rotation, refresh tokens, and secure storage.', 'Knowledge', .84, .71, days(1, 15)],
  ['m4', 'Raw meeting transcript', 'Unprocessed notes from the design review with unresolved questions and quotes.', 'Raw', .48, .24, days(1, 9)],
  ['m5', 'Prefer semantic motion', 'Pulse only when freshness is meaningful; decorative loops create noise.', 'Wisdom', .96, .82, days(2, 18)],
  ['m6', 'Timeline uses spatial time', 'A dot position on the twenty-four-hour axis should replace repeated time labels.', 'Decision', .88, .76, days(3, 13)],
  ['m7', 'Parser benchmark result', 'The new tokenizer reduced context scaffolding by nineteen percent.', 'Knowledge', .79, .62, days(5, 20)],
  ['m8', 'Loose idea: project heatmap', 'Consider a ninety-day capture heatmap above the daily time tracks.', 'Raw', .42, .31, days(7, 8)],
  ['m9', 'Bento hierarchy rule', 'Tile area maps to impact while trust remains a continuous ring.', 'Decision', .9, .86, days(12, 16)],
  ['m10', 'Database backup path', 'The local database lives under the Nexus application data directory.', 'Knowledge', .99, .45, days(20, 10)],
  ['m11', 'Do not redesign navigation chrome', 'Sidebar and topbar are stable context. Redesign content pages only.', 'Wisdom', .93, .78, days(33, 14)],
  ['m12', 'Unsorted capture', 'A rough capture that still needs verification and a useful summary.', 'Raw', .33, .16, days(48, 19)],
].map(([id, title, summary, layer, confidence, importance, createdAt]) => ({
  id, title, summary, content: `${summary}\n\nThis is longer verbatim content used to exercise the reading surface and prove the information hierarchy under realistic text lengths.`,
  layer, confidenceScore: confidence, importanceScore: importance, createdAt, updatedAt: createdAt,
  author: id === 'm1' ? 'copilot' : 'manual', source: id === 'm1' ? 'Nexus MCP' : 'Manual',
  visibility: 'Private', captureMode: 'manual', projectSpaceId: null,
  linkedEntityIds: id === 'm1' ? ['e1', 'e2'] : [], latestVersionId: null, status: 'active',
  attachedFiles: id === 'm1' ? [{ name: 'context-spec.md', path: 'C:\\Nexus\\context-spec.md', sizeBytes: 4210, mimeType: 'text/markdown' }] : [],
}));

const contextDto = {
  id: 'ctx-1',
  user_intent: { query: 'How should the Nexus context page explain package assembly?', intent_type: 'explain_and_decide', confidence: .91 },
  entities: [
    { id: 'e1', entityType: 'project', title: 'Nexus', description: 'Local-first cognitive memory system', status: 'active', createdAt: days(50), updatedAt: hours(2) },
    { id: 'e2', entityType: 'concept', title: 'Context Package', description: 'Ranked and compressed material passed to a model', status: 'active', createdAt: days(30), updatedAt: hours(3) },
    { id: 'e3', entityType: 'technology', title: 'Tauri', description: 'Desktop application runtime', status: 'active', createdAt: days(80), updatedAt: days(5) },
  ],
  memory_records: memories.slice(0, 5),
  relationships: [
    { id: 'r1', sourceEntityId: 'e1', targetEntityId: 'e2', relationshipType: 'produces', weight: .9, createdAt: days(4) },
    { id: 'r2', sourceEntityId: 'e1', targetEntityId: 'e3', relationshipType: 'runs_on', weight: .7, createdAt: days(9) },
  ],
  created_at: hours(1), token_count: 1840,
  provenance: {
    traces: [
      { id: 'e2', kind: 'entity', title: 'Context Package', reasons: [{ kind: 'queryMatch', query: 'context page' }, { kind: 'keywordMatch', keyword: 'package' }], score: .94, scoreParts: [{ component: 'query', points: .6 }, { component: 'graph', points: .34 }], tokens: 120, included: true, dropped: null },
      { id: 'm1', kind: 'memory', title: 'Context assembly should explain itself', reasons: [{ kind: 'memorySearch', query: 'context assembly' }, { kind: 'highImportance', importance: .94 }], score: .91, scoreParts: [{ component: 'semantic', points: .7 }, { component: 'importance', points: .21 }], tokens: 260, included: true, dropped: null },
      { id: 'm2', kind: 'memory', title: 'Nexus page redesign direction', reasons: [{ kind: 'keywordMatch', keyword: 'redesign' }, { kind: 'recentActivity', ageDays: 0 }], score: .77, scoreParts: [{ component: 'keyword', points: .5 }, { component: 'recency', points: .27 }], tokens: 230, included: true, dropped: null },
      { id: 'm8', kind: 'memory', title: 'Loose idea: project heatmap', reasons: [{ kind: 'keywordMatch', keyword: 'page' }], score: .22, scoreParts: [{ component: 'keyword', points: .22 }], tokens: 190, included: false, dropped: { kind: 'belowRelevance', score: .22, floor: .35 } },
      { id: 'e3', kind: 'entity', title: 'Tauri', reasons: [{ kind: 'graphExpansion', fromId: 'e1', fromTitle: 'Nexus', hops: 1 }], score: .31, scoreParts: [{ component: 'graph', points: .31 }], tokens: 80, included: false, dropped: { kind: 'tokenBudget', limit: 2000 } },
    ],
  },
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ memoryRows, context }) => {
    const internals = {
      invoke: async (cmd: string) => {
        if (cmd === 'setup_needed') return false;
        if (cmd === 'get_memories') return memoryRows;
        if (cmd === 'get_all_config') return [{ key: 'app.theme', value: 'dark' }, { key: 'app.language', value: localStorage.getItem('qa_locale') ?? 'en' }];
        if (cmd === 'get_config') return null;
        if (cmd === 'get_graph') return { nodes: context.entities, edges: context.relationships };
        if (cmd === 'get_projects') return [];
        if (cmd === 'check_stale_projects') return [];
        if (cmd === 'build_context') return context;
        if (cmd === 'export_context') return { content: '# Nexus context\n\nMock exported package', format: 'markdown', tokens: 24, tokenMethod: 'exact', filename: 'nexus-context.md' };
        return null;
      },
      transformCallback: () => 1,
      unregisterCallback: () => undefined,
      runCallback: () => undefined,
      callbacks: new Map(),
    };
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: internals, configurable: true });
  }, { memoryRows: memories, context: contextDto });

  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('.st-page')).toBeVisible();
  await expect(page.locator('.st-page button button')).toHaveCount(0);
});

test('Memories Strata bento and detail', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1050 });
  await expect(page.locator('.st-strata-segment')).toHaveCount(4);
  await expect(page.locator('.st-memory-tile')).toHaveCount(memories.length);
  await page.screenshot({ path: 'test-results/visual/memories-strata.png', fullPage: true });

  await page.locator('.st-memory-tile').first().click();
  await expect(page.locator('.st-sheet')).toBeVisible();
  await expect(page.locator('.st-ladder-step')).toHaveCount(4);
  await page.screenshot({ path: 'test-results/visual/memory-detail.png', fullPage: true });
});

test('Timeline heatmap and 24-hour tracks', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1050 });
  await page.locator('.sidebar').getByRole('button', { name: /^Timeline/ }).click();
  await expect(page.locator('.st-heat-cell')).toHaveCount(90);
  await expect(page.locator('.st-axis-dot').first()).toBeVisible();
  await page.screenshot({ path: 'test-results/visual/timeline-tracks.png', fullPage: true });
});

test('Context Assembly pipeline explains all stages', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1200 });
  await page.locator('.sidebar').getByRole('button', { name: /^Context/ }).click();
  await expect(page.locator('.st-stage')).toHaveCount(7);
  await page.locator('.st-ask-field input').fill('How should context assembly work?');
  await page.locator('.st-run').click();
  await expect(page.locator('.st-rank-row')).toHaveCount(contextDto.provenance.traces.length);
  await page.screenshot({ path: 'test-results/visual/context-assembly.png', fullPage: true });
});

test('Responsive pages do not overflow', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 1000 });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  expect(overflow).toBe(false);
  await page.screenshot({ path: 'test-results/visual/memories-responsive.png', fullPage: true });
});

test('DOM geometry is intentional and Russian copy does not overflow', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 1000 });

  const widths = await page.locator('.st-memory-tile').evaluateAll((tiles) =>
    [...new Set(tiles.map((tile) => Math.round(tile.getBoundingClientRect().width)))],
  );
  // The wall must be bento, not another equal-card grid.
  expect(widths.length).toBeGreaterThanOrEqual(3);

  const inspectOverflow = async () => page.locator('.st-page').evaluate((root) => {
    const rootRect = root.getBoundingClientRect();
    const selectors = [
      '.st-hero', '.st-strata-panel', '.st-legend', '.st-rail', '.st-bento',
      '.st-memory-tile', '.st-heat-panel', '.st-day', '.st-ask', '.st-stage-card',
    ].join(',');
    return [...root.querySelectorAll<HTMLElement>(selectors)]
      .filter((element) => {
        const rect = element.getBoundingClientRect();
        return rect.width > 2 && (rect.right > rootRect.right + 3 || rect.left < rootRect.left - 3);
      })
      .map((element) => ({
        tag: element.tagName,
        className: element.className,
        scrollWidth: element.scrollWidth,
        clientWidth: element.clientWidth,
      }));
  });

  expect(await inspectOverflow()).toEqual([]);

  await page.evaluate(() => localStorage.setItem('qa_locale', 'ru'));
  await page.reload();
  await page.waitForLoadState('networkidle');
  await expect(page.getByText('Воспоминания').first()).toBeVisible();
  expect(await inspectOverflow()).toEqual([]);
  await page.screenshot({ path: 'test-results/visual/memories-russian.png', fullPage: true });
});
