/**
 * Copy for the three content pages: Memories, Timeline, Context.
 *
 * Split out of `localeStore` the same way `setupLocale` and `contextLocale` are,
 * so the prose can be read and reviewed as prose instead of hunting it out of a
 * 400-line dictionary.
 *
 * `pagesRu` is typed as `Record<keyof typeof pagesEn, string>`, so forgetting a
 * Russian key is a build error rather than a page that silently falls back to
 * English. The fallback in `localeStore.t` exists for keys added by other
 * modules; it should never be what ships here.
 *
 * A note on layer names: `Raw` / `Knowledge` / `Decision` / `Wisdom` are values
 * stored in the database, not UI labels, so they are shown verbatim in both
 * locales. What gets translated is the *meaning* line next to them — the user
 * needs to know what "Wisdom" is for, and that explanation is prose.
 */

export const pagesEn = {
  // ── Layer semantics ───────────────────────────────────────────────────────
  // Every layer answers two questions: what is this, and what would move it up.
  'layer.raw.meaning': 'Captured as-is. Nothing has been checked or condensed yet.',
  'layer.raw.promotes': 'Verify it and it becomes Knowledge.',
  'layer.knowledge.meaning': 'A fact that held up to checking and can be reused.',
  'layer.knowledge.promotes': 'Act on it and the choice becomes a Decision.',
  'layer.decision.meaning': 'A choice that was made, with the reasoning behind it.',
  'layer.decision.promotes': 'Hold across projects and it hardens into Wisdom.',
  'layer.wisdom.meaning': 'A principle that outlived the case that produced it.',
  'layer.wisdom.promotes': 'Top of the ladder — this is the durable form.',

  'layer.ladder': 'Maturity',
  'layer.ladder.hint':
    'Memories climb Raw → Knowledge → Decision → Wisdom. The rung tells you how much processing has gone into a record, not how important it is.',
  'layer.stage': 'Stage',

  // ── Shared instrument vocabulary ──────────────────────────────────────────
  'inst.trust': 'Trust',
  'inst.trust.hint':
    'How sure Nexus is that this is correct. A ring, because it is a fraction of certainty — it can sit anywhere between empty and full.',
  'inst.impact': 'Impact',
  'inst.impact.hint':
    'How much this matters. Shown as five blocks because impact is a rank, not a measurement — and the tile itself grows with it.',
  'inst.explain': 'What is this?',
  'inst.of': 'of',

  // ── Memories ──────────────────────────────────────────────────────────────
  'mem.hero.sub':
    'Everything Nexus has retained. Bigger tile means it matters more, a filled ring means it is trusted more, and a beating dot means it arrived today.',
  'mem.hero.kicker': 'Nexus / memory field',
  'mem.stats.records': 'Records',
  'mem.stats.avgImpact': 'Avg impact',
  'mem.strata.title': 'Composition',
  'mem.strata.hint':
    'The shape of your collection across the maturity ladder. Click a band to keep only that layer.',
  'mem.legend.title': 'What the layers mean',

  'mem.sort': 'Order',
  'mem.sort.recent': 'Newest',
  'mem.sort.impact': 'Impact',
  'mem.sort.trust': 'Trust',
  'mem.sort.title': 'A–Z',
  'mem.view': 'Density',
  'mem.view.bento': 'Sized',
  'mem.view.list': 'Rows',
  'mem.view.hint':
    'Sized lays tiles out by impact so the important records take more room. Rows gives every memory the same line, for scanning a long list.',

  'mem.fresh': 'Today',
  'mem.recent': 'This week',
  'mem.settled': 'Settled',
  'mem.pulse.hint': 'Beating dot: captured in the last 24 hours.',

  'mem.open': 'Open',
  'mem.filtered': 'shown',
  'mem.clear': 'Clear filters',
  'mem.none.title': 'Nothing matches',
  'mem.none.desc': 'No memory fits those filters. Clear the search or widen the layer selection.',
  'mem.empty.desc':
    'Memories arrive from the MCP server or the copilot. Open the command bar and run create-memory to add the first one by hand.',

  // ── Memory sheet ──────────────────────────────────────────────────────────
  'sheet.back': 'All memories',
  'sheet.copy': 'Copy',
  'sheet.copied': 'Copied',
  'sheet.summary': 'In short',
  'sheet.summary.hint': 'The human-written gist. Written by whoever captured this.',
  'sheet.content': 'Verbatim',
  'sheet.content.hint':
    'Exactly what a model receives when this memory enters a context package. Shown in monospace because it is data, not prose.',
  'sheet.files': 'Attached',
  'sheet.chars': 'chars',
  'sheet.expand': 'Show all',
  'sheet.collapse': 'Collapse',
  'sheet.created': 'Captured',
  'sheet.updated': 'Updated',
  'sheet.origin': 'Origin',
  'sheet.author': 'Author',
  'sheet.visibility': 'Visibility',
  'sheet.capture': 'Captured by',
  'sheet.status': 'Status',
  'sheet.linked': 'Linked entities',
  'sheet.provenance': 'Where this came from',

  // ── Memory trust lifecycle ─────────────────────────────────────────────────
  'sheet.lifecycle': 'Trust',
  'sheet.lifecycle.hint':
    'How trustworthy this record is right now. Nexus starts everything at Inferred; your Confirm is what promotes a record to a fact you can build on.',
  'sheet.state.current': 'Current',
  'sheet.state.inferred': 'Inferred',
  'sheet.state.conflicted': 'Conflicted',
  'sheet.state.superseded': 'Superseded',
  'sheet.state.userConfirmed': 'User confirmed',
  'sheet.state.hint.current': 'Taken as correct until something contradicts it.',
  'sheet.state.hint.inferred': 'Derived by the model, not yet verified by a human.',
  'sheet.state.hint.conflicted': 'Contradicts other memories — needs a decision.',
  'sheet.state.hint.superseded': 'Replaced by a newer record. Kept for history.',
  'sheet.state.hint.userConfirmed': 'You explicitly verified this record.',
  'sheet.confirm': 'Confirm',
  'sheet.confirm.done': 'Confirmed',
  'sheet.confirm.hint': 'Mark this memory as verified. Confirmed records are treated as facts by the copilot.',
  'sheet.confirmed.by': 'Confirmed by',
  'sheet.supersedes': 'Supersedes',
  'sheet.superseded.by': 'Superseded by',
  'sheet.feedback.title': 'Was it useful?',
  'sheet.feedback.hint': 'Each vote feeds the memory-quality metrics. Marking a record Wrong flags it as conflicted. Click a verdict, explain why, then confirm — the explanation is stored on the memory and used by the copilot.',
  'sheet.feedback.useful': 'Useful',
  'sheet.feedback.irrelevant': 'Irrelevant',
  'sheet.feedback.wrong': 'Wrong',
  'sheet.feedback.note.title': 'Tell the copilot why',
  'sheet.feedback.note.hint': 'Your explanation is stored on the memory and used by the copilot and semantic search to understand what is right, missing, or misleading.',
  'sheet.feedback.note.placeholder': "What's right, what's missing, what should change?",
  'sheet.feedback.note.send': 'Send',
  'sheet.feedback.note.sent': 'Saved',
  'sheet.feedback.note.saved': 'Explanation saved',
  'sheet.feedback.note.close': 'Close',

  // ── Timeline ──────────────────────────────────────────────────────────────
  'tl.hero.sub':
    'Capture history. The strip below is your last three months at a glance; each day opens into a 24-hour track where a memory sits at the hour it arrived.',
  'tl.hero.kicker': 'Nexus / memory time',
  'tl.stats.entries': 'Entries',
  'tl.stats.days': 'Active days',
  'tl.heat.title': 'Last 90 days',
  'tl.heat.hint':
    'One cell per day, brighter where more was captured. Click a cell to jump to that day.',
  'tl.axis.hint':
    'Each day is a 24-hour track running midnight to midnight. A dot sits at the hour it was captured, takes its colour from its layer, and grows with its impact.',
  'tl.axis.morning': '06',
  'tl.axis.noon': '12',
  'tl.axis.evening': '18',
  'tl.order': 'Direction',
  'tl.order.newest': 'Newest first',
  'tl.order.oldest': 'Oldest first',
  'tl.day.one': 'memory',
  'tl.day.many': 'memories',
  'tl.quiet': 'Nothing captured',
  'tl.busiest': 'Busiest day',
  'tl.span': 'Span',
  'tl.empty.desc': 'Once memories exist they line up here by day, newest at the top.',
  'tl.none.title': 'Nothing in these layers',
  'tl.none.desc': 'Clear the layer filters to see the whole history.',
  'tl.progress': 'Read',
  'tl.heat.aria': 'Ninety-day memory activity',
  'tl.captured': 'captured',

  // ── Context — the pipeline ────────────────────────────────────────────────
  'ctx.hero.sub':
    'A context package is what Nexus hands a model instead of your whole database. Ask a question below and watch it get assembled — every stage shows what it did and why.',
  'ctx.hero.kicker': 'Nexus / context assembly',
  'ctx.stats.selected': 'Selected',

  'ctx.ask.label': 'Ask',
  'ctx.ask.placeholder': 'Ask it the way you would ask a colleague…',
  'ctx.ask.hint': 'Press Enter to assemble',
  'ctx.ask.run': 'Assemble',
  'ctx.seeds': 'or try',

  'ctx.pipeline.title': 'How it gets built',
  'ctx.pipeline.idle':
    'These seven stages run on every question. They are dimmed until you ask something — then each one fills with what it actually did.',

  'ctx.stage.query.name': 'Your question',
  'ctx.stage.query.desc': 'The text you typed. Everything below is derived from it.',
  'ctx.stage.intent.name': 'Reading the intent',
  'ctx.stage.intent.desc':
    'Nexus classifies what kind of question this is, because a recall question and a decision question need different material.',
  'ctx.stage.gather.name': 'Gathering candidates',
  'ctx.stage.gather.desc':
    'It pulls entities from the knowledge graph and records from memory, then walks the relationships outward from what it found.',
  'ctx.stage.rank.name': 'Scoring',
  'ctx.stage.rank.desc':
    'Each candidate earns points for matching your words, for being recent, for being important, for how close it sits in the graph.',
  'ctx.stage.prune.name': 'Cutting',
  'ctx.stage.prune.desc':
    'What scored too low, or would not fit the token budget, is dropped here. This is the stage nobody else shows you.',
  'ctx.stage.pack.name': 'The package',
  'ctx.stage.pack.desc':
    'What survived, and what it costs in tokens. That cost is the whole point: a smaller package that says the same thing is a cheaper answer.',
  'ctx.stage.export.name': 'Handing it over',
  'ctx.stage.export.desc':
    'Take the package to any model. Markdown to paste into a chat, JSON for a program, Plain when every token of formatting is a cost you pay.',

  'ctx.intent.type': 'Question type',
  'ctx.intent.confidence': 'Sure of it',
  'ctx.gather.entities': 'Entities',
  'ctx.gather.memories': 'Memories',
  'ctx.gather.links': 'Relationships',
  'ctx.gather.from': 'from',
  'ctx.rank.score': 'Score',
  'ctx.rank.breakdown': 'Why it scored that',
  'ctx.rank.expand': 'Show the arithmetic',
  'ctx.rank.none': 'This backend did not return a scoring trace. The package still works, but the arithmetic cannot be explained for this run.',
  'ctx.prune.none': 'Nothing was cut — everything found fit the budget.',
  'ctx.prune.count': 'cut',
  'ctx.pack.tokens': 'Tokens',
  'ctx.pack.budget': 'Budget used',
  'ctx.pack.share': 'of the package',
  'ctx.kept': 'Kept',
  'ctx.dropped': 'Cut',
  'ctx.clear': 'Start over',
  'ctx.empty.title': 'Nothing assembled yet',
  'ctx.empty.desc': 'Ask a question above. The stages will fill in as it runs.',
  'ctx.working': 'Assembling…',
  'ctx.state.working': 'working',
  'ctx.state.ready': 'ready',
  'ctx.state.waiting': 'waiting',
  'ctx.stage.placeholder': 'Ask a question above. This stage will fill with its actual result.',
  'ctx.intent.type.note': 'What the question asks Nexus to retrieve.',
  'ctx.intent.confidence.note': 'How sure the classification is.',
  'ctx.source.entities.note': 'Nodes from the knowledge graph',
  'ctx.source.memories.note': 'Records matched from memory',
  'ctx.source.links.note': 'Edges connecting the result',
  'ctx.reason.query': 'query match',
  'ctx.reason.keyword': 'keyword',
  'ctx.reason.graph': 'graph distance',
  'ctx.reason.memory': 'memory search',
  'ctx.reason.recent': 'recent',
  'ctx.reason.important': 'important',
  'ctx.drop.budget': 'token budget',
  'ctx.drop.cap': 'entity cap',
  'ctx.drop.relevance': 'below relevance',

  // ── Score arithmetic — the copilot's formula ──────────────────────────────
  'ctx.score.titleMatch': 'Title match',
  'ctx.score.keywordMatch': 'Keyword match',
  'ctx.score.contentMatch': 'Content match',
  'ctx.score.importance': 'Importance',
  'ctx.score.recency': 'Recency',
  'ctx.score.confidence': 'Confidence',
  'ctx.score.base': 'Base',
  'ctx.showInGraph': 'Show in graph',
} as const;

export type PagesKey = keyof typeof pagesEn;

export const pagesRu: Record<PagesKey, string> = {
  // ── Семантика слоёв ───────────────────────────────────────────────────────
  'layer.raw.meaning': 'Записано как есть. Ещё не проверено и не сжато.',
  'layer.raw.promotes': 'Проверьте — станет Knowledge.',
  'layer.knowledge.meaning': 'Факт, который выдержал проверку и годится для повторного использования.',
  'layer.knowledge.promotes': 'Примите на его основе решение — станет Decision.',
  'layer.decision.meaning': 'Принятое решение вместе с обоснованием.',
  'layer.decision.promotes': 'Если работает и в других проектах — затвердеет в Wisdom.',
  'layer.wisdom.meaning': 'Принцип, переживший случай, который его породил.',
  'layer.wisdom.promotes': 'Верх лестницы — это уже устойчивая форма.',

  'layer.ladder': 'Зрелость',
  'layer.ladder.hint':
    'Воспоминания идут по лестнице Raw → Knowledge → Decision → Wisdom. Ступень говорит, сколько обработки прошла запись, а не насколько она важна.',
  'layer.stage': 'Ступень',

  // ── Общий язык приборов ───────────────────────────────────────────────────
  'inst.trust': 'Доверие',
  'inst.trust.hint':
    'Насколько Nexus уверен, что это верно. Показано кольцом, потому что это доля уверенности — может быть любой между пустым и полным.',
  'inst.impact': 'Вес',
  'inst.impact.hint':
    'Насколько это важно. Показано пятью блоками, потому что вес — это ранг, а не измерение. И сама плитка растёт вместе с ним.',
  'inst.explain': 'Что это?',
  'inst.of': 'из',

  // ── Воспоминания ──────────────────────────────────────────────────────────
  'mem.hero.sub':
    'Всё, что Nexus сохранил. Плитка крупнее — значит важнее, кольцо заполнено — значит больше доверия, точка пульсирует — значит запись появилась сегодня.',
  'mem.hero.kicker': 'Nexus / поле памяти',
  'mem.stats.records': 'Записей',
  'mem.stats.avgImpact': 'Средний вес',
  'mem.strata.title': 'Состав',
  'mem.strata.hint':
    'Форма вашей коллекции по лестнице зрелости. Нажмите на полосу, чтобы оставить только этот слой.',
  'mem.legend.title': 'Что означают слои',

  'mem.sort': 'Порядок',
  'mem.sort.recent': 'Новые',
  'mem.sort.impact': 'Вес',
  'mem.sort.trust': 'Доверие',
  'mem.sort.title': 'А–Я',
  'mem.view': 'Плотность',
  'mem.view.bento': 'По весу',
  'mem.view.list': 'Строки',
  'mem.view.hint':
    '«По весу» раскладывает плитки так, что важные записи занимают больше места. «Строки» дают каждой записи одинаковую строку — удобно просматривать длинный список.',

  'mem.fresh': 'Сегодня',
  'mem.recent': 'На этой неделе',
  'mem.settled': 'Устоялось',
  'mem.pulse.hint': 'Пульсирующая точка: записано за последние 24 часа.',

  'mem.open': 'Открыть',
  'mem.filtered': 'показано',
  'mem.clear': 'Сбросить фильтры',
  'mem.none.title': 'Ничего не подходит',
  'mem.none.desc': 'Под эти фильтры ничего не попало. Очистите поиск или добавьте слоёв.',
  'mem.empty.desc':
    'Воспоминания приходят с MCP-сервера или от копилота. Откройте командную строку и выполните create-memory, чтобы добавить первое вручную.',

  // ── Карточка памяти ───────────────────────────────────────────────────────
  'sheet.back': 'Все воспоминания',
  'sheet.copy': 'Скопировать',
  'sheet.copied': 'Скопировано',
  'sheet.summary': 'Коротко',
  'sheet.summary.hint': 'Суть, написанная человеком — тем, кто сохранял эту запись.',
  'sheet.content': 'Дословно',
  'sheet.content.hint':
    'Ровно то, что получает модель, когда эта запись попадает в пакет контекста. Моношрифт — потому что это данные, а не проза.',
  'sheet.files': 'Вложения',
  'sheet.chars': 'симв.',
  'sheet.expand': 'Показать всё',
  'sheet.collapse': 'Свернуть',
  'sheet.created': 'Записано',
  'sheet.updated': 'Изменено',
  'sheet.origin': 'Источник',
  'sheet.author': 'Автор',
  'sheet.visibility': 'Видимость',
  'sheet.capture': 'Способ записи',
  'sheet.status': 'Статус',
  'sheet.linked': 'Связанные сущности',
  'sheet.provenance': 'Откуда это взялось',

  // ── Жизненный цикл доверия ─────────────────────────────────────────────────
  'sheet.lifecycle': 'Доверие',
  'sheet.lifecycle.hint':
    'Насколько этой записи можно верить прямо сейчас. Nexus начинает всё со статуса «Выведено»; ваше подтверждение переводит запись в факт, на который можно опираться.',
  'sheet.state.current': 'Актуально',
  'sheet.state.inferred': 'Выведено',
  'sheet.state.conflicted': 'Противоречит',
  'sheet.state.superseded': 'Заменено',
  'sheet.state.userConfirmed': 'Подтверждено',
  'sheet.state.hint.current': 'Считается верным, пока ничто не противоречит.',
  'sheet.state.hint.inferred': 'Получено моделью, человеком пока не проверено.',
  'sheet.state.hint.conflicted': 'Противоречит другим записям — нужно решение.',
  'sheet.state.hint.superseded': 'Заменено более новой записью. Хранится для истории.',
  'sheet.state.hint.userConfirmed': 'Вы явно проверили эту запись.',
  'sheet.confirm': 'Подтвердить',
  'sheet.confirm.done': 'Подтверждено',
  'sheet.confirm.hint': 'Пометить запись как проверенную. Подтверждённые записи копилот считает фактами.',
  'sheet.confirmed.by': 'Подтвердил',
  'sheet.supersedes': 'Заменяет',
  'sheet.superseded.by': 'Заменено на',
  'sheet.feedback.title': 'Было полезно?',
  'sheet.feedback.hint': 'Каждый голос идёт в метрики качества памяти. Пометка «Неверно» переводит запись в конфликт. Нажмите на оценку, объясните, почему, и подтвердите — объяснение сохранится в записи и будет использовано копилотом.',
  'sheet.feedback.useful': 'Полезно',
  'sheet.feedback.irrelevant': 'Неактуально',
  'sheet.feedback.wrong': 'Неверно',
  'sheet.feedback.note.title': 'Объясни копилоту, почему',
  'sheet.feedback.note.hint': 'Ваше объяснение хранится на записи и используется копилотом и семантическим поиском, чтобы понимать, что верно, чего не хватает и что вводит в заблуждение.',
  'sheet.feedback.note.placeholder': 'Что верно, чего не хватает, что стоит изменить?',
  'sheet.feedback.note.send': 'Отправить',
  'sheet.feedback.note.sent': 'Сохранено',
  'sheet.feedback.note.saved': 'Объяснение сохранено',
  'sheet.feedback.note.close': 'Закрыть',

  // ── Хронология ────────────────────────────────────────────────────────────
  'tl.hero.sub':
    'История записи. Полоса ниже — последние три месяца одним взглядом; каждый день раскрывается в 24-часовую дорожку, где запись стоит на том часе, когда пришла.',
  'tl.hero.kicker': 'Nexus / время памяти',
  'tl.stats.entries': 'Записей',
  'tl.stats.days': 'Активных дней',
  'tl.heat.title': 'Последние 90 дней',
  'tl.heat.hint':
    'Одна клетка — один день, ярче там, где записано больше. Нажмите на клетку, чтобы перейти к этому дню.',
  'tl.axis.hint':
    'Каждый день — дорожка на 24 часа, от полуночи до полуночи. Точка стоит на часе записи, берёт цвет своего слоя и растёт вместе с весом.',
  'tl.axis.morning': '06',
  'tl.axis.noon': '12',
  'tl.axis.evening': '18',
  'tl.order': 'Направление',
  'tl.order.newest': 'Сначала новые',
  'tl.order.oldest': 'Сначала старые',
  'tl.day.one': 'запись',
  'tl.day.many': 'записей',
  'tl.quiet': 'Ничего не записано',
  'tl.busiest': 'Самый плотный день',
  'tl.span': 'Охват',
  'tl.empty.desc': 'Как только появятся воспоминания, они выстроятся здесь по дням — новые сверху.',
  'tl.none.title': 'В этих слоях ничего нет',
  'tl.none.desc': 'Снимите фильтры по слоям, чтобы увидеть всю историю.',
  'tl.progress': 'Прочитано',
  'tl.heat.aria': 'Активность памяти за девяносто дней',
  'tl.captured': 'сохранено',

  // ── Контекст — конвейер ───────────────────────────────────────────────────
  'ctx.hero.sub':
    'Пакет контекста — это то, что Nexus отдаёт модели вместо всей вашей базы. Задайте вопрос ниже и посмотрите, как он собирается: каждая стадия показывает, что она сделала и почему.',
  'ctx.hero.kicker': 'Nexus / сборка контекста',
  'ctx.stats.selected': 'Отобрано',

  'ctx.ask.label': 'Вопрос',
  'ctx.ask.placeholder': 'Спросите так, как спросили бы коллегу…',
  'ctx.ask.hint': 'Enter — собрать',
  'ctx.ask.run': 'Собрать',
  'ctx.seeds': 'или попробуйте',

  'ctx.pipeline.title': 'Как это собирается',
  'ctx.pipeline.idle':
    'Эти семь стадий выполняются на каждый вопрос. Пока вы ничего не спросили, они приглушены — потом каждая заполнится тем, что действительно сделала.',

  'ctx.stage.query.name': 'Ваш вопрос',
  'ctx.stage.query.desc': 'Текст, который вы ввели. Всё ниже выводится из него.',
  'ctx.stage.intent.name': 'Чтение намерения',
  'ctx.stage.intent.desc':
    'Nexus определяет тип вопроса: «вспомнить» и «решить» требуют разного материала.',
  'ctx.stage.gather.name': 'Сбор кандидатов',
  'ctx.stage.gather.desc':
    'Берёт сущности из графа знаний и записи из памяти, затем расходится по связям от найденного.',
  'ctx.stage.rank.name': 'Оценка',
  'ctx.stage.rank.desc':
    'Каждый кандидат набирает баллы: за совпадение со словами запроса, за свежесть, за важность, за близость в графе.',
  'ctx.stage.prune.name': 'Отсев',
  'ctx.stage.prune.desc':
    'То, что набрало слишком мало или не влезло в бюджет токенов, отбрасывается здесь. Эту стадию не показывает больше никто.',
  'ctx.stage.pack.name': 'Пакет',
  'ctx.stage.pack.desc':
    'Что осталось и сколько это стоит в токенах. Стоимость — весь смысл: меньший пакет с тем же содержанием даёт более дешёвый ответ.',
  'ctx.stage.export.name': 'Передача',
  'ctx.stage.export.desc':
    'Отнесите пакет любой модели. Markdown — вставить в чат, JSON — для программы, Plain — когда каждый токен разметки вы оплачиваете.',

  'ctx.intent.type': 'Тип вопроса',
  'ctx.intent.confidence': 'Уверенность',
  'ctx.gather.entities': 'Сущности',
  'ctx.gather.memories': 'Воспоминания',
  'ctx.gather.links': 'Связи',
  'ctx.gather.from': 'из',
  'ctx.rank.score': 'Балл',
  'ctx.rank.breakdown': 'Из чего сложился балл',
  'ctx.rank.expand': 'Показать арифметику',
  'ctx.rank.none': 'Этот backend не вернул трассировку оценки. Пакет работает, но арифметику этого запуска показать нельзя.',
  'ctx.prune.none': 'Ничего не отсеяно — всё найденное влезло в бюджет.',
  'ctx.prune.count': 'отсеяно',
  'ctx.pack.tokens': 'Токены',
  'ctx.pack.budget': 'Бюджет израсходован',
  'ctx.pack.share': 'от пакета',
  'ctx.kept': 'Оставлено',
  'ctx.dropped': 'Отсеяно',
  'ctx.clear': 'Начать заново',
  'ctx.empty.title': 'Пакет пока не собран',
  'ctx.empty.desc': 'Задайте вопрос выше. Стадии заполнятся по ходу сборки.',
  'ctx.working': 'Собираю…',
  'ctx.state.working': 'работает',
  'ctx.state.ready': 'готово',
  'ctx.state.waiting': 'ожидает',
  'ctx.stage.placeholder': 'Задайте вопрос выше. Эта стадия заполнится фактическим результатом.',
  'ctx.intent.type.note': 'Какой материал Nexus должен достать для этого вопроса.',
  'ctx.intent.confidence.note': 'Насколько система уверена в классификации.',
  'ctx.source.entities.note': 'Узлы из графа знаний',
  'ctx.source.memories.note': 'Записи, найденные в памяти',
  'ctx.source.links.note': 'Связи между найденным',
  'ctx.reason.query': 'совпадение с запросом',
  'ctx.reason.keyword': 'ключевое слово',
  'ctx.reason.graph': 'расстояние в графе',
  'ctx.reason.memory': 'поиск по памяти',
  'ctx.reason.recent': 'свежее',
  'ctx.reason.important': 'важное',
  'ctx.drop.budget': 'бюджет токенов',
  'ctx.drop.cap': 'лимит сущностей',
  'ctx.drop.relevance': 'ниже порога',

  // ── Арифметика балла — формула копилота ───────────────────────────────────
  'ctx.score.titleMatch': 'Совпадение заголовка',
  'ctx.score.keywordMatch': 'Совпадение ключевых слов',
  'ctx.score.contentMatch': 'Совпадение содержимого',
  'ctx.score.importance': 'Важность',
  'ctx.score.recency': 'Свежесть',
  'ctx.score.confidence': 'Уверенность',
  'ctx.score.base': 'Базовая оценка',
  'ctx.showInGraph': 'Показать в графе',
};
