# Changelog

Все заметные изменения Ru-Meetily документируются в этом файле.

Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/), версионирование — [SemVer](https://semver.org/lang/ru/).

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

[0.4.1]: https://github.com/Godila/Ru-Meetily/releases/tag/v0.4.1
[0.4.0]: https://github.com/Godila/Ru-Meetily/releases/tag/v0.4.0
