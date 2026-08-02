/**
 * Copy for the context provenance panel and the export bar.
 *
 * Split out of `localeStore` for the same reason the setup wizard's copy was:
 * these strings are prose that has to be read as prose to be reviewed. The
 * `ContextCopy` type forces both locales to carry the same keys, so a missing
 * Russian string is a compile error rather than a silent fallback to English in
 * the middle of an explanation.
 *
 * Wording note: the panel exists to make ranking auditable, so the copy names
 * the actual mechanism ("came through the graph from X") instead of vague
 * praise ("highly relevant"). A user who disagrees with a verdict should be able
 * to tell *which* step to blame.
 */

export type ContextCopy = {
  // ── Provenance panel ──
  'why.title': string;
  'why.subtitle': string;
  'why.tab.included': string;
  'why.tab.dropped': string;
  'why.empty.included': string;
  'why.empty.dropped': string;

  // Reasons for inclusion
  'why.reason.queryMatch': string;
  'why.reason.keywordMatch': string;
  'why.reason.graphExpansion': string;
  'why.reason.memorySearch': string;
  'why.reason.recentActivity': string;
  'why.reason.highImportance': string;
  'why.reason.today': string;

  // Reasons for exclusion
  'why.drop.belowRelevance': string;
  'why.drop.tokenBudget': string;
  'why.drop.entityCap': string;

  // Score breakdown components
  'why.part.titleMatch': string;
  'why.part.contentMatch': string;
  'why.part.keywordMatch': string;
  'why.part.importance': string;
  'why.part.confidence': string;
  'why.part.recency': string;
  'why.part.base': string;

  // Units
  'why.hop': string;
  'why.hops': string;
  'why.days': string;

  // ── Export ──
  'export.title': string;
  'export.subtitle': string;
  'export.markdown': string;
  'export.markdownHint': string;
  'export.json': string;
  'export.jsonHint': string;
  'export.plain': string;
  'export.plainHint': string;
  'export.working': string;
  'export.copy': string;
  'export.copied': string;
  'export.save': string;
  'export.saved': string;
  'export.tokens': string;
  'export.exact': string;
  'export.estimated': string;
  'export.preview': string;
};

export const contextEn: ContextCopy = {
  'why.title': 'Why this is in your context',
  'why.subtitle':
    'Every item below earned its place. Expand one to see the arithmetic that ranked it.',
  'why.tab.included': 'Included',
  'why.tab.dropped': 'Left out',
  'why.empty.included': 'Nothing was included. Try a broader query.',
  'why.empty.dropped': 'Nothing was left out — everything found fitted the budget.',

  'why.reason.queryMatch': 'Matched your query',
  'why.reason.keywordMatch': 'Matched keyword',
  'why.reason.graphExpansion': 'Reached through the graph from',
  'why.reason.memorySearch': 'Found by searching your memories',
  'why.reason.recentActivity': 'Changed recently',
  'why.reason.highImportance': 'You marked this important',
  'why.reason.today': 'Changed today',

  'why.drop.belowRelevance': 'Scored below the relevance floor',
  'why.drop.tokenBudget': 'Did not fit the token budget',
  'why.drop.entityCap': 'Cut by the entity limit',

  'why.part.titleMatch': 'title match',
  'why.part.contentMatch': 'content match',
  'why.part.keywordMatch': 'keyword match',
  'why.part.importance': 'importance',
  'why.part.confidence': 'confidence',
  'why.part.recency': 'recency',
  'why.part.base': 'base',

  'why.hop': 'hop',
  'why.hops': 'hops',
  'why.days': 'days ago',

  'export.title': 'Send this context anywhere',
  'export.subtitle':
    'Nexus builds the package; any model can consume it. Not tied to one provider.',
  'export.markdown': 'Markdown',
  'export.markdownHint': 'Paste into ChatGPT, Claude, Gemini',
  'export.json': 'JSON',
  'export.jsonHint': 'For your own tooling',
  'export.plain': 'Plain',
  'export.plainHint': 'Facts only, cheapest to send',
  'export.working': 'Building…',
  'export.copy': 'Copy',
  'export.copied': 'Copied',
  'export.save': 'Save to file',
  'export.saved': 'Saved',
  'export.tokens': 'tokens',
  'export.exact': 'measured with the real vocabulary',
  'export.estimated': 'estimated — model cache unavailable',
  'export.preview': 'Preview',
};

export const contextRu: ContextCopy = {
  'why.title': 'Почему это попало в контекст',
  'why.subtitle':
    'Каждый элемент ниже попал сюда не случайно. Разверните любой, чтобы увидеть, как считался балл.',
  'why.tab.included': 'Включено',
  'why.tab.dropped': 'Отброшено',
  'why.empty.included': 'Ничего не включено. Попробуйте более широкий запрос.',
  'why.empty.dropped': 'Ничего не отброшено — всё найденное поместилось в бюджет.',

  'why.reason.queryMatch': 'Совпало с запросом',
  'why.reason.keywordMatch': 'Совпало по слову',
  'why.reason.graphExpansion': 'Пришло по графу от',
  'why.reason.memorySearch': 'Найдено поиском по памяти',
  'why.reason.recentActivity': 'Недавно изменялось',
  'why.reason.highImportance': 'Вы отметили как важное',
  'why.reason.today': 'Изменялось сегодня',

  'why.drop.belowRelevance': 'Балл ниже порога релевантности',
  'why.drop.tokenBudget': 'Не поместилось в бюджет токенов',
  'why.drop.entityCap': 'Срезано лимитом сущностей',

  'why.part.titleMatch': 'совпадение в заголовке',
  'why.part.contentMatch': 'совпадение в тексте',
  'why.part.keywordMatch': 'совпадение по слову',
  'why.part.importance': 'важность',
  'why.part.confidence': 'достоверность',
  'why.part.recency': 'свежесть',
  'why.part.base': 'база',

  'why.hop': 'шаг',
  'why.hops': 'шага',
  'why.days': 'дн. назад',

  'export.title': 'Отправьте контекст в любую модель',
  'export.subtitle':
    'Nexus собирает пакет — использовать его может любая модель. Без привязки к одному поставщику.',
  'export.markdown': 'Markdown',
  'export.markdownHint': 'Вставить в ChatGPT, Claude, Gemini',
  'export.json': 'JSON',
  'export.jsonHint': 'Для своих инструментов',
  'export.plain': 'Простой текст',
  'export.plainHint': 'Только факты, дешевле всего отправлять',
  'export.working': 'Собираем…',
  'export.copy': 'Скопировать',
  'export.copied': 'Скопировано',
  'export.save': 'Сохранить в файл',
  'export.saved': 'Сохранено',
  'export.tokens': 'токенов',
  'export.exact': 'измерено реальным словарём',
  'export.estimated': 'оценка — словарь модели недоступен',
  'export.preview': 'Предпросмотр',
};
