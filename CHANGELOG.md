# Changelog

Все заметные изменения Convoic (ранее Ru-Meetily) документируются в этом файле.

Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/), версионирование — [SemVer](https://semver.org/lang/ru/).

## [0.5.1] — 2026-07-25

### 🎨 Бренд-айдентика: логотип Convoic

Добавлен фирменный логотип Convoic во все точки контакта с пользователем.

### ✨ Что нового

- **Логотип в окне «О программе»** (`About.tsx`) — использует transparent-вариант `convoic_icon_128.png` на белом фоне диалога, увеличен до 96px для лучшей читаемости. Прежнее `icon_128x128.png` (upstream) было сломано (файл отсутствовал в `public/`).
- **Логотип в Sidebar collapsed** (`Logo.tsx`) — вместо несуществующего `/logo-collapsed.png` теперь используется `convoic_icon_128.png` 36×36px с правильным выравниванием по центру.
- **favicon и apple-touch-icon** — настроены в `metadata.ts`/`metadata.tsx` через Next.js `icons` field. Браузер/Tauri-окно теперь показывает фирменную иконку Convoic вместо дефолтной Next.js.
- **Bundle-иконки (Windows installer/taskbar/title bar)** — все 26 файлов в `src-tauri/icons/` перегенерированы из master-логотипа `convoic_icon_1024_white.png` (white-background вариант выбран для корректного отображения на тёмной теме Windows taskbar):
  - 24 PNG (32/128/256, Square30..310, StoreLogo, icon_16..512 серия)
  - 2 multi-size ICO (`icon.ico`, `app_icon.ico`) — 6 размеров внутри (16/32/48/64/128/256)

### 🛠️ Технически

- Backup старых иконок создан в `src-tauri/icons_backup_pre_convoic/` (для отката при необходимости)
- Источник лого: `frontend/public/convoic_icon_*.png` (transparent) и `convoic_icon_*_white.png` (white BG) — пользователь генерировал бренд-набор
- `.icns` (macOS) оставлены старые — требуют macOS tooling для перегенерации (не блокирует Windows release)

---

## [0.5.0] — 2026-07-25

### 🎉 Ребрендинг: Ru-Meetily → Convoic

Полный ребрендинг приложения. Новое имя, новый identifier, новая версия.

**Convoic = convo (разговор) + voice (голос)** — «голос ваших встреч, превращённый в смысл». Tagline: *Convoic. The voice of your conversations.*

Имя выбрано через направленный brainstorm из следующих соображений: самостоятельный неологизм на латинице, корневая метафора «встреча + голос», технологичный AI-first тон, 7 букв / 2 слога, легко произносится по-русски (Конво́ик) и по-английски (/ˈkɒn.vɔɪk/). Brand spec: `docs/brand/convoic-brand-spec.md`.

### ⚠️ Breaking changes

- **Имя приложения:** `Ru-Meetily` → `Convoic`
- **App identifier:** `com.meetily.ai` → `com.convoic.app`
- **AppData path:** `%APPDATA%\com.meetily.ai\` → `%APPDATA%\com.convoic.app\`
- **Cargo package name:** `meetily` → `convoic` (бинарь: `meetily.exe` → `convoic.exe`)
- **Без миграции данных:** существующие встречи, модели GigaAM (~227 MB) и настройки из 0.4.x НЕ переносятся. При первом запуске Convoic стартует с чистого листа. Старое приложение Ru-Meetily остаётся установленным, его можно удалить вручную вместе с `%APPDATA%\com.meetily.ai\`.
- **Папка записей:** `~/Music/meetily-recordings` → `~/Music/convoic-recordings`
- **Templates subpath:** `%APPDATA%\Roaming\Meetily\templates` → `%APPDATA%\Roaming\Convoic\templates`
- **Env-vars (для dev/CI):** `MEETILY_LLAMA_HELPER` → `CONVOIC_LLAMA_HELPER`, `MEETILY_SKIP_SIDECAR_VERIFY` → `CONVOIC_SKIP_SIDECAR_VERIFY`

### ✨ Что нового

- Все UI-строки (About, Info, Logo, Sidebar, Onboarding, dialogs, metadata) обновлены под бренд Convoic
- Tray tooltip и все notification titles локализованы под Convoic
- Метаданные приложения (`productName`, `identifier`, `version`, `Cargo.toml` name + description + repository + authors, `package.json` name) приведены в соответствие
- Tauri window title, build script'ы (.bat/.ps1), Windows installer locale — обновлены
- README полностью переработан под новый бренд с migration notice для существующих пользователей
- Storage keys переименованы: `meetily_user_id` → `convoic_user_id` (sessionStorage), `MeetilyRecoveryDB` → `ConvoicRecoveryDB` (IndexedDB)
- macOS CoreAudio tap name и Console.app process identifier переименованы (косметика, не влияет на Windows-сборку)

### 🛠️ Осознанные исключения (out of scope)

- **Backend Python-сервис** (`backend/` с Docker-compose, Homebrew keg names) — отдельный сервис, не входит в Tauri bundle. Оставлен как есть.
- **Parakeet model download URL** (`https://meetily.towardsgeneralintelligence.com/...`) — upstream-инфраструктура, работает. Переименование сломало бы скачивание.
- **Legacy DB detection paths** (`/opt/homebrew/var/meetily/...`, `/usr/local/var/meetily/...`) — функциональный detection старых upstream-инсталляций для импорта. Переименование сломало бы import-flow.
- **GitHub repo** переименован: `Godila/Ru-Meetily` → `Godila/convoic` (старые ссылки автоматически редиректят).
- **Логотип/айдентика, домен `convoic.ai`, товарный знак** — отдельные задачи вне кодовой базы.

### 📦 Известные ограничения

- Установщики не подписаны код-сертификатом (`DIGICERT_KEYPAIR_ALIAS` не задан) — Windows SmartScreen выдаст предупреждение
- Auto-update через Tauri updater не настроен (`TAURI_SIGNING_PRIVATE_KEY` отсутствует) — обновление только ручной переустановкой

---

## [0.4.1] — 2026-07-25

### Главные изменения

Релиз полностью посвящён русскоязычному UX: убрана «двойная механика» генерации (EN → RU-перевод), переведён весь пользовательский интерфейс, локализовано системное меню трея и починен давний UX-баг, при котором генерация саммари «сбрасывалась» при уходе со страницы встречи.

### ✨ Добавлено

- **Фоновая генерация саммари переживает навигацию.** Генерация больше не отменяется при переходе в «Настройки» или другую встречу — поллинг живёт в глобальном провайдере и завершается сам.
- **Индикатор прогресса в Sidebar.** На карточке встречи, пока идёт генерация, показывается вращающийся спиннер с тултипом «Генерация резюме...».
- **Авто-resume при возврате.** При повторном открытии встречи с незавершённой генерацией спиннер и поллинг запускаются автоматически — без необходимости перезапускать задачу.
- **Прямая генерация на русском.** Базовый системный промпт теперь явно требует «Пиши саммари/отчёт на русском языке; текст на других языках недопустим». Заголовки секций шаблона также локализуются.
- **Полная локализация UI на русский (P1–P3):** флоу саммари, менеджеры моделей (Whisper/llama.cpp/Parakeet/GigaAM), диалоги подтверждения, пустые состояния, тосты и подписи.

### ♻️ Изменено

- **Трей-меню полностью на русском:** «Начать запись», «Пауза», «Остановить», «Продолжить», «Открыть окно», «Настройки», «Выход». Тултип переименован в «Ru-Meetily».
- **UX-полировка переводов** как у нативного продукта для рынка РФ: убраны машинные формулировки, половинчатые переводы, страдательный залог, англицизмы; консистентная терминология («саммари», «резюме», «транскрипция», «инференс», «модель», «устройство»).
- **Подпись переключателя GPU-инференса** исправлена: убрано ложное «CPU точнее», формулировка финализирована как «GPU (быстрее) / CPU (стабильнее)».

### 🗑️ Удалено

- **Механика выбора языка саммари** (легаси en→ru): удалены UI-пикеры, хук `useRecentLanguages`, библиотека `summary-languages`, типы `FinalLanguageAction`, функции `translate_markdown`, `normalize_markdown_to_english`, `run_markdown_transform`, `resolve_cached_english`, `resolve_final_language_action`, `translation_system_prompt`, и 5 Tauri-команд (`api_get/save_meeting_summary_language`, `api_get/save_meeting_detected_summary_language`, `api_detect_transcript_summary_language`, `api_process_transcript` очищен от параметра `summary_language`).
- **Зависимость `whatlang`** из `Cargo.toml` (раньше использовалась для определения языка транскрипции).
- **Пункт «Проверить обновления»** из системного трея.
- Удалённые файлы: `summary/language_detection.rs`, `summary/metadata.rs`, `SummaryLanguageSettings.tsx`, `LanguagePickerPopover.tsx`, `useRecentLanguages.ts`, `summary-languages.ts`, `summary-language-preferences.ts` + 2 unit-теста к ним.

### 🐛 Исправлено

- **Qwen3 thinking-mode leakage:** утечка разметки `<think>...</think>` в финальное саммари. Модель теперь корректно разделяет reasoning и итоговый ответ.
- **Генерация отменялась при навигации:** багованный cleanup-эффект в `meeting-details/page.tsx` убивал polling при любом unmount-е, включая уход в «Настройки». Заменён на стабильную ref-only архитектуру без state+ref-зеркала.
- **Cross-meeting contamination:** auto-resume callback мог записать саммари встречи A на страницу встречи B. Добавлен `currentMeetingIdRef` guard.
- **Зависший спиннер при resume:** локальный `summaryStatus` не обновлялся при завершении генерации на resume-пути. Добавлен subscription-эффект, синхронизирующий локальный статус с глобальным.
- **Interval leak window:** state+ref mirror рассинхронизировался при быстром рестарте. Переведено на чистый `useRef<Map>` — callbacks стабильны (`useCallback([])`), эффекты не дёргаются.

### 🔧 Технически

- **Архитектура polling lifecycle:** `activeSummaryPolls` убран из React state и контекста, переведён в `activeSummaryPollsRef: useRef<Map>`. Глобальный статус саммари поднят в `SidebarProvider` как `summaryStatuses: Record<meetingId, SummaryStatus>` с хелпером `backendStatusToSummaryStatus`.
- **Тип `SummaryStatus`** вынесен в общий экспорт из `SidebarProvider` (`'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error'`), используется в Sidebar, useSummaryGeneration и meeting-details.
- **Тесты:** 170/170 Rust unit-тестов, 50/50 Vitest-тестов, 11/11 Next.js страниц собираются, `tsc --noEmit` — чисто, clippy по модулям PR — без замечаний.

### 📦 В составе PR #5

`fix/qwen-thinking-mode-and-ux` (8 коммитов + merge):

- `f59a188` fix: Qwen3 thinking-mode leakage + UX локализация и подписи
- `ee47081` fix(ui): убрать ложное «CPU точнее» из подписи переключателя GPU
- `a26e380` fix(ui): финальная подпись переключателя GPU
- `218a894` feat(summary): прямой отказ от en→ru — генерация саммари сразу на русском
- `c50548e` refactor: удалить легаси выбора языка саммари (UI + хуки + бэкенд)
- `b533af6` i18n: локализация английских UI-строк на русский (P1+P2+P3)
- `3c91d24` fix(i18n): UX-полировка русских переводов
- `44ddbab` feat(tray): локализация меню трея + удаление «Проверить обновления»
- `cd6b901` fix(summary): фоновая генерация переживает навигацию + спиннер в Sidebar
- `daedc40` fix(code-review): адресация замечаний ревью (polling lifecycle + i18n)
- `e5ab3cc` Merge PR #5

## [0.4.0] — 2025

Базовая версия форка Ru-Meetily (унаследована от upstream Meetily):
- Интеграция GigaAM STT для русского языка
- Умный онбординг + настройка GPU-инференса (PR #4)
- Vulkan GPU-инференс для llama-helper на Windows (PR #3)

[0.5.1]: https://github.com/Godila/convoic/releases/tag/v0.5.1
[0.5.0]: https://github.com/Godila/convoic/releases/tag/v0.5.0
[0.4.1]: https://github.com/Godila/convoic/releases/tag/v0.4.1
[0.4.0]: https://github.com/Godila/convoic/releases/tag/v0.4.0
