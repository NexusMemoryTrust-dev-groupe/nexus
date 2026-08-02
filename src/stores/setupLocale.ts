/**
 * Translations for the first-run setup wizard.
 *
 * Kept in its own module rather than inlined into `localeStore` so the wizard's
 * copy can be reviewed as prose. Every key exists in both locales; a missing
 * Russian key silently falls back to English in `localeStore.t`, which is
 * exactly the failure mode we want to avoid in an onboarding flow, so the
 * `SetupCopy` type below forces both maps to stay in step.
 */

export type SetupCopy = {
  // Shell
  'setup.title': string;
  'setup.subtitle': string;
  'setup.progress': string;
  'setup.recheck': string;
  'setup.back': string;
  'setup.next': string;
  'setup.finish': string;
  'setup.skip': string;
  'setup.skipHint': string;
  'setup.working': string;
  'setup.retry': string;
  'setup.openFolder': string;
  'setup.copy': string;
  'setup.copied': string;

  // Status words
  'setup.status.ok': string;
  'setup.status.missing': string;
  'setup.status.checking': string;
  'setup.status.failed': string;

  // Step names
  'setup.step.welcome': string;
  'setup.step.node': string;
  'setup.step.opencode': string;
  'setup.step.key': string;
  'setup.step.model': string;
  'setup.step.mcp': string;
  'setup.step.done': string;

  // Welcome
  'setup.welcome.heading': string;
  'setup.welcome.body': string;
  'setup.welcome.point1': string;
  'setup.welcome.point2': string;
  'setup.welcome.point3': string;
  'setup.welcome.time': string;
  'setup.welcome.start': string;

  // Node
  'setup.node.heading': string;
  'setup.node.why': string;
  'setup.node.found': string;
  'setup.node.absent': string;
  'setup.node.how1': string;
  'setup.node.how2': string;
  'setup.node.how3': string;
  'setup.node.download': string;

  // OpenCode
  'setup.opencode.heading': string;
  'setup.opencode.why': string;
  'setup.opencode.found': string;
  'setup.opencode.absent': string;
  'setup.opencode.install': string;
  'setup.opencode.installing': string;
  'setup.opencode.manual': string;

  // API key
  'setup.key.heading': string;
  'setup.key.why': string;
  'setup.key.free': string;
  'setup.key.present': string;
  'setup.key.absent': string;
  'setup.key.placeholder': string;
  'setup.key.save': string;
  'setup.key.saved': string;
  'setup.key.whereHeading': string;
  'setup.key.where1': string;
  'setup.key.where2': string;
  'setup.key.where3': string;
  'setup.key.where4': string;
  'setup.key.test': string;
  'setup.key.testing': string;
  'setup.key.testOk': string;
  'setup.key.testFail': string;
  'setup.key.privacy': string;

  // Model
  'setup.model.heading': string;
  'setup.model.why': string;
  'setup.model.freeBadge': string;
  'setup.model.recommended': string;
  'setup.model.current': string;
  'setup.model.loading': string;
  'setup.model.none': string;

  // MCP
  'setup.mcp.heading': string;
  'setup.mcp.why': string;
  'setup.mcp.registered': string;
  'setup.mcp.absent': string;
  'setup.mcp.stale': string;
  'setup.mcp.register': string;
  'setup.mcp.registering': string;
  'setup.mcp.configAt': string;
  'setup.mcp.sandbox': string;

  // Done
  'setup.done.heading': string;
  'setup.done.body': string;
  'setup.done.next1': string;
  'setup.done.next2': string;
  'setup.done.next3': string;
  'setup.done.launch': string;
  'setup.done.partial': string;

  // Provenance of the token figures shown on the final step.
  'setup.tokens.exact': string;
  'setup.tokens.estimated': string;
};

export const setupEn: SetupCopy = {
  'setup.tokens.exact': 'Exact token counting is active — savings are measured, not estimated.',
  'setup.tokens.estimated':
    'Token counts are approximated until the model vocabulary downloads. Savings stay directional until then.',
  'setup.title': 'Welcome to Nexus',
  'setup.subtitle': 'A few checks and your memory is ready to use.',
  'setup.progress': 'Step {current} of {total}',
  'setup.recheck': 'Check again',
  'setup.back': 'Back',
  'setup.next': 'Continue',
  'setup.finish': 'Open Nexus',
  'setup.skip': 'Skip for now',
  'setup.skipHint': 'You can finish setup later in Settings.',
  'setup.working': 'Working...',
  'setup.retry': 'Try again',
  'setup.openFolder': 'Show folder',
  'setup.copy': 'Copy',
  'setup.copied': 'Copied',

  'setup.status.ok': 'Ready',
  'setup.status.missing': 'Not found',
  'setup.status.checking': 'Checking',
  'setup.status.failed': 'Failed',

  'setup.step.welcome': 'Welcome',
  'setup.step.node': 'Node.js',
  'setup.step.opencode': 'OpenCode',
  'setup.step.key': 'API key',
  'setup.step.model': 'Model',
  'setup.step.mcp': 'Connection',
  'setup.step.done': 'Done',

  'setup.welcome.heading': 'Nexus remembers so your AI does not have to re-read everything',
  'setup.welcome.body':
    'Nexus keeps your notes, files and decisions as a knowledge graph. When an AI asks a question, Nexus sends only the relevant part instead of whole documents, which is what makes answers cheaper and sharper.',
  'setup.welcome.point1': 'Your data stays on this computer, in a local database.',
  'setup.welcome.point2': 'Any AI that speaks MCP can read your memory, with your permission.',
  'setup.welcome.point3': 'Token savings are measured, not estimated.',
  'setup.welcome.time': 'This takes about two minutes.',
  'setup.welcome.start': 'Get started',

  'setup.node.heading': 'Node.js',
  'setup.node.why':
    'The OpenCode command-line tool runs on Node.js. Nexus uses it to talk to AI models.',
  'setup.node.found': 'Node.js is installed.',
  'setup.node.absent': 'Node.js was not found on this computer.',
  'setup.node.how1': 'Open nodejs.org and download the LTS version.',
  'setup.node.how2': 'Run the installer and accept the defaults.',
  'setup.node.how3': 'Come back here and press "Check again".',
  'setup.node.download': 'Open nodejs.org',

  'setup.opencode.heading': 'OpenCode CLI',
  'setup.opencode.why':
    'OpenCode connects Nexus to AI models, including several free ones. Nexus never sends your data anywhere on its own.',
  'setup.opencode.found': 'OpenCode is installed.',
  'setup.opencode.absent': 'OpenCode is not installed yet.',
  'setup.opencode.install': 'Install OpenCode',
  'setup.opencode.installing': 'Installing OpenCode. This can take a minute.',
  'setup.opencode.manual': 'Prefer to do it yourself? Run this in a terminal:',

  'setup.key.heading': 'API key',
  'setup.key.why':
    'The key lets OpenCode reach an AI model. It is stored locally and never leaves this computer except in requests you make.',
  'setup.key.free': 'You can skip this and use a free model, then add a key later.',
  'setup.key.present': 'A key is already saved.',
  'setup.key.absent': 'No key saved yet.',
  'setup.key.placeholder': 'Paste your key here',
  'setup.key.save': 'Save key',
  'setup.key.saved': 'Key saved.',
  'setup.key.whereHeading': 'Where to get a key',
  'setup.key.where1': 'Open opencode.ai/auth in your browser.',
  'setup.key.where2': 'Sign in, then choose "Create API key".',
  'setup.key.where3': 'Copy the key. It looks like a long line starting with "sk-".',
  'setup.key.where4': 'Paste it into the field above and press "Save key".',
  'setup.key.test': 'Test connection',
  'setup.key.testing': 'Asking the model to reply...',
  'setup.key.testOk': 'The model answered. Everything works.',
  'setup.key.testFail': 'The model did not answer.',
  'setup.key.privacy': 'The key is stored in your local Nexus database.',

  'setup.model.heading': 'Choose a model',
  'setup.model.why': 'This is the model the built-in copilot will use. You can change it any time.',
  'setup.model.freeBadge': 'free',
  'setup.model.recommended': 'recommended',
  'setup.model.current': 'Currently selected',
  'setup.model.loading': 'Loading the list of models...',
  'setup.model.none':
    'Could not load the model list. You can continue and pick a model later in Settings.',

  'setup.mcp.heading': 'Connect Nexus to your AI',
  'setup.mcp.why':
    'This registers Nexus with OpenCode as a memory server. After this, an AI can search your memory, read your files and build context on its own.',
  'setup.mcp.registered': 'Nexus is connected.',
  'setup.mcp.absent': 'Nexus is not connected yet.',
  'setup.mcp.stale': 'The connection points at an old location and needs updating.',
  'setup.mcp.register': 'Connect now',
  'setup.mcp.registering': 'Connecting...',
  'setup.mcp.configAt': 'Configuration file',
  'setup.mcp.sandbox':
    'The AI can only read and change files inside folders you add to Nexus. Everything else on your disk is off limits.',

  'setup.done.heading': 'You are set',
  'setup.done.body': 'Nexus is ready. Your memory lives on this computer and is yours alone.',
  'setup.done.next1': 'Add a folder or project so Nexus can index it.',
  'setup.done.next2': 'Ask the copilot something about your own notes.',
  'setup.done.next3': 'Open Savings to see measured token savings as you work.',
  'setup.done.launch': 'Open Nexus',
  'setup.done.partial':
    'Some steps were skipped. Nexus works, but AI features stay off until you finish them in Settings.',
};

export const setupRu: SetupCopy = {
  'setup.tokens.exact': 'Точный подсчёт токенов включён — экономия измеряется, а не оценивается.',
  'setup.tokens.estimated':
    'Пока словарь модели не загружен, токены считаются приблизительно. До этого экономия показывает направление, а не точную цифру.',
  'setup.title': 'Добро пожаловать в Nexus',
  'setup.subtitle': 'Несколько проверок — и память готова к работе.',
  'setup.progress': 'Шаг {current} из {total}',
  'setup.recheck': 'Проверить снова',
  'setup.back': 'Назад',
  'setup.next': 'Продолжить',
  'setup.finish': 'Открыть Nexus',
  'setup.skip': 'Пропустить',
  'setup.skipHint': 'Настройку можно завершить позже в разделе «Настройки».',
  'setup.working': 'Выполняется...',
  'setup.retry': 'Повторить',
  'setup.openFolder': 'Показать папку',
  'setup.copy': 'Копировать',
  'setup.copied': 'Скопировано',

  'setup.status.ok': 'Готово',
  'setup.status.missing': 'Не найдено',
  'setup.status.checking': 'Проверяем',
  'setup.status.failed': 'Ошибка',

  'setup.step.welcome': 'Начало',
  'setup.step.node': 'Node.js',
  'setup.step.opencode': 'OpenCode',
  'setup.step.key': 'Ключ API',
  'setup.step.model': 'Модель',
  'setup.step.mcp': 'Подключение',
  'setup.step.done': 'Готово',

  'setup.welcome.heading': 'Nexus помнит, чтобы ИИ не перечитывал всё заново',
  'setup.welcome.body':
    'Nexus хранит ваши заметки, файлы и решения как граф знаний. Когда ИИ задаёт вопрос, Nexus отдаёт только нужную часть, а не целые документы — поэтому ответы точнее и дешевле.',
  'setup.welcome.point1': 'Данные остаются на этом компьютере, в локальной базе.',
  'setup.welcome.point2': 'Любой ИИ с поддержкой MCP может читать вашу память — с вашего разрешения.',
  'setup.welcome.point3': 'Экономия токенов измеряется, а не прикидывается.',
  'setup.welcome.time': 'Это займёт около двух минут.',
  'setup.welcome.start': 'Начать',

  'setup.node.heading': 'Node.js',
  'setup.node.why':
    'Утилита OpenCode работает на Node.js. Через неё Nexus общается с моделями ИИ.',
  'setup.node.found': 'Node.js установлен.',
  'setup.node.absent': 'Node.js на этом компьютере не найден.',
  'setup.node.how1': 'Откройте nodejs.org и скачайте версию LTS.',
  'setup.node.how2': 'Запустите установщик и оставьте настройки по умолчанию.',
  'setup.node.how3': 'Вернитесь сюда и нажмите «Проверить снова».',
  'setup.node.download': 'Открыть nodejs.org',

  'setup.opencode.heading': 'OpenCode CLI',
  'setup.opencode.why':
    'OpenCode связывает Nexus с моделями ИИ, включая несколько бесплатных. Сам Nexus никуда ничего не отправляет.',
  'setup.opencode.found': 'OpenCode установлен.',
  'setup.opencode.absent': 'OpenCode пока не установлен.',
  'setup.opencode.install': 'Установить OpenCode',
  'setup.opencode.installing': 'Устанавливаем OpenCode. Это может занять минуту.',
  'setup.opencode.manual': 'Хотите сделать вручную? Выполните в терминале:',

  'setup.key.heading': 'Ключ API',
  'setup.key.why':
    'Ключ нужен, чтобы OpenCode обратился к модели. Он хранится локально и не покидает компьютер, кроме ваших собственных запросов.',
  'setup.key.free': 'Этот шаг можно пропустить и работать на бесплатной модели, а ключ добавить позже.',
  'setup.key.present': 'Ключ уже сохранён.',
  'setup.key.absent': 'Ключ пока не сохранён.',
  'setup.key.placeholder': 'Вставьте ключ сюда',
  'setup.key.save': 'Сохранить ключ',
  'setup.key.saved': 'Ключ сохранён.',
  'setup.key.whereHeading': 'Где взять ключ',
  'setup.key.where1': 'Откройте в браузере opencode.ai/auth.',
  'setup.key.where2': 'Войдите и выберите «Create API key».',
  'setup.key.where3': 'Скопируйте ключ — это длинная строка, начинается на «sk-».',
  'setup.key.where4': 'Вставьте его в поле выше и нажмите «Сохранить ключ».',
  'setup.key.test': 'Проверить подключение',
  'setup.key.testing': 'Просим модель ответить...',
  'setup.key.testOk': 'Модель ответила. Всё работает.',
  'setup.key.testFail': 'Модель не ответила.',
  'setup.key.privacy': 'Ключ хранится в локальной базе Nexus.',

  'setup.model.heading': 'Выберите модель',
  'setup.model.why':
    'Эту модель будет использовать встроенный копилот. Её можно поменять в любой момент.',
  'setup.model.freeBadge': 'бесплатно',
  'setup.model.recommended': 'рекомендуем',
  'setup.model.current': 'Выбрана сейчас',
  'setup.model.loading': 'Загружаем список моделей...',
  'setup.model.none':
    'Не удалось загрузить список моделей. Можно продолжить и выбрать модель позже в настройках.',

  'setup.mcp.heading': 'Подключите Nexus к вашему ИИ',
  'setup.mcp.why':
    'Nexus зарегистрируется в OpenCode как сервер памяти. После этого ИИ сможет сам искать по вашей памяти, читать файлы и собирать контекст.',
  'setup.mcp.registered': 'Nexus подключён.',
  'setup.mcp.absent': 'Nexus пока не подключён.',
  'setup.mcp.stale': 'Подключение указывает на старое расположение и требует обновления.',
  'setup.mcp.register': 'Подключить',
  'setup.mcp.registering': 'Подключаем...',
  'setup.mcp.configAt': 'Файл конфигурации',
  'setup.mcp.sandbox':
    'ИИ может читать и менять файлы только внутри папок, добавленных в Nexus. Остальной диск для него закрыт.',

  'setup.done.heading': 'Готово',
  'setup.done.body': 'Nexus настроен. Память хранится на этом компьютере и принадлежит только вам.',
  'setup.done.next1': 'Добавьте папку или проект, чтобы Nexus его проиндексировал.',
  'setup.done.next2': 'Спросите копилота о своих же заметках.',
  'setup.done.next3': 'Откройте «Экономию», чтобы видеть измеренную экономию токенов.',
  'setup.done.launch': 'Открыть Nexus',
  'setup.done.partial':
    'Часть шагов пропущена. Nexus работает, но функции ИИ останутся отключёнными, пока вы их не завершите в настройках.',
};
