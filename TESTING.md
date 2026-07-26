# TESTING — рекомендации по быстрой итерации без переустановки бандла

Этот документ — для разработки Ru-Meetily на Windows. Цель: перестать
переустанавливать `.exe`/`.msi` после каждого изменения.

## TL;DR — повседневный цикл

```bash
cd frontend
pnpm run tauri:dev      # НЕ clean_run_windows.bat
```

Держи процесс запущенным. Frontend (Next.js) hot-reload'ится сам; Rust
пересобирается инкрементально и перезапускает приложение (~30сек–минута на
правку core-модуля, секунды на правку изолированной функции). **Никогда не
вызывай `clean_run_windows.bat` для рутинной разработки** — он стирает
`node_modules` и lockfile, добавляя 1–3 минуты на каждый запуск.

## Главные находки (что не так сейчас)

1. **`clean_run_windows.bat` вреден для итераций.** Стирает `node_modules`,
   удаляет `package-lock.json` (не `pnpm-lock.yaml`!), и не выставляет
   `RUST_LOG`. Используй только для случая «deps сломаны намертво».
2. **`RUST_LOG` в `main.rs:10` форсится в `info`.** Документированный в
   `CLAUDE.md` способ `$env:RUST_LOG="debug"; ./clean_run_windows.bat`
   **не работает** — `main()` безусловно перезаписывает переменную.
3. **Портативный `.exe` уже существует** после `pnpm run tauri:build`:
   `frontend/src-tauri/target/release/Ru-Meetily.exe`. Можно запускать
   **без установки**. (Нужны рядом sidecar-бинари `binaries/ffmpeg.exe`,
   `binaries/llama-helper.exe` и bundled `templates/*.json` — они уже в
   `target/release/` после сборки.)
4. **Rust hot-reload невозможен** — фундаментальное ограничение Tauri.
   Но это компенсируется юнит-тестами (см. ниже).

## Карта инструментов тестирования

### Rust-тесты (быстро, без приложения)

В кодобазе **190 тестов** в 36 файлах под `frontend/src-tauri/src/`. Запуск:

```bash
# все
cargo test --manifest-path frontend/src-tauri/Cargo.toml

# конкретный модуль (секунды)
cargo test --manifest-path frontend/src-tauri/Cargo.toml summary::templates

# с выводом println!
cargo test --manifest-path frontend/src-tauri/Cargo.toml summary::templates -- --nocapture
```

**Паттерн для итераций:** прежде чем трогать функцию, напиши/расширь её
юнит-тест. Тогда цикл «правка → проверка» занимает секунды через `cargo test`,
а не минут через перезапуск приложения.

### Интеграционный тест с реальным аудио (GigaAM)

Два готовых harness-теста:

**VAD-проверка** (декодер + silero-VAD, без модели): шаблон
`frontend/src-tauri/src/audio/import.rs:1312`
(`test_import_pipeline_decode_vad`, `#[ignore]`). Берёт аудио из env, декодирует,
гонит через VAD с разными `redemption_time` и печатает статистику сегментов.

**GigaAM-транскрипция** (полный пайплайн до текста): тест
`gigaam_engine::tests::test_transcribe_real_audio` в
`frontend/src-tauri/src/gigaam_engine/gigaam_engine.rs`. Использует
`GigaamEngine::new_with_models_dir(Option<PathBuf>)` — конструктор без
`AppHandle`, поэтому работает в изолированном тесте. Грузит модель
`gigaam-v3-rnnt-int8`, гонит 16kHz-mono аудио через RNN-T и печатает транскрипт +
RTF (real-time factor).

Запуск GigaAM-harness:

```bash
TEST_AUDIO_PATH=C:/path/to/sample.wav \
GIGAAM_MODELS_DIR=C:/Users/geor/AppData/Roaming/Meetily/models \
  cargo test --manifest-path frontend/src-tauri/Cargo.toml \
    gigaam_engine::tests::test_transcribe_real_audio -- \
    --ignored --nocapture
```

Env-переменные:
- `TEST_AUDIO_PATH` (обязательно) — путь к аудио (wav/mp4/m4a/…; декодер
  использует FFmpeg sidecar).
- `GIGAAM_MODELS_DIR` (опц.) — **родитель** поддиректории `gigaam` с моделями.
  По умолчанию движок берёт платформенный AppData (на Windows это
  `%APPDATA%\Meetily\models`). Передавай, если модели лежат в нестандартном
  месте. Движок сам добавит `gigaam` к пути.
- `GIGAAM_MODEL_NAME` (опц.) — имя модели из каталога; по умолчанию
  `gigaam-v3-rnnt-int8`.

Тест требует, чтобы модель уже была скачана (через UI приложения) — он её не
докачивает. Если модель не `Available`, тест упадёт с понятной диагностикой.

### Frontend

Тесты — **vitest** + `@testing-library/react` + jsdom. 57 тестов в 6 файлах:

```bash
cd frontend
pnpm test                  # разовый прогон
pnpm test:watch            # watch-режим
pnpm test:ui               # браузерный UI vitest
pnpm run typecheck:tests   # типы тестов (tsconfig.vitest.json)
```

Покрытие:
- `src/lib/__tests__/utils.test.ts` — `cn`, `isOllamaNotInstalledError`
- `src/lib/__tests__/summary-languages.test.ts` — `normaliseLanguageCode`,
  `labelForCode`, константы
- `src/lib/__tests__/summary-language-preferences.test.ts` — pinned default
  localStorage (чтение/запись/normalisation)
- `src/lib/__tests__/blocknote-markdown.test.ts` — `blocksToMarkdownSafely`
  (успех/fallback/без fallback)
- `src/lib/__tests__/onboarding-summary-model.test.ts` — выбор модели онбординга
  и размеры
- `src/hooks/meeting-details/__tests__/useTemplates.test.ts` — хук CRUD:
  initial load, selection, editor lifecycle, create/update/delete/json (invoke
  замокан через `vi.hoisted` + `vi.mock('@tauri-apps/api/core')`)

**Паттерн для тестирования Tauri-хуков:** мокаем `@tauri-apps/api/core` →
`invoke` через `vi.hoisted` (обязательно, иначе хойстинг `vi.mock` ломает
связь), stub'им `sonner` (порталы) и любой transitive-импорт с Tauri-вызовами
(`@/lib/analytics`). Mount-effects флешим через `await setTimeout(50)` внутри
`act`, а НЕ `vi.waitFor` (с ним бывают рейсы с React-batching).

Конфигурация:
- `vitest.config.ts` — jsdom env, `@/` alias (зеркало tsconfig `paths`),
  setup `vitest.setup.ts` (jest-dom matchers, matchMedia/ResizeObserver stubs)
- `tsconfig.json` — исключает тесты и vitest-файлы (Next.js их не компилирует)
- `tsconfig.vitest.json` — включает только тесты с vitest-глобалами
- `tsconfig.ci.json` — для CI: src без тестов

Next.js dev-сервер на `localhost:3118` можно открывать в обычном браузере для
отладки UI/CSS — но любой путь, вызывающий `invoke('...')`, упадёт вне Tauri
webview. Для такой логики используй vitest с моками.

### DevTools

В debug-сборке (`tauri dev`) — `Ctrl+Shift+I` открывает DevTools.
В release-сборке DevTools недоступны (Cargo feature `devtools` не подключён).

## Рекомендуемые точечные улучшения (отдельные PR'ы)

По возрастанию выгоды:

1. **Фикс `RUST_LOG` override** — 1 строка в `main.rs:10`:
   ```rust
   if env::var("RUST_LOG").is_err() { std::env::set_var("RUST_LOG", "info"); }
   ```
   Та же правка в `console_utils.rs:34`. После этого `$env:RUST_LOG="debug"`
   реально работает.

2. **CI на PR** — сейчас все 8 workflow под `.github/workflows/` — manual
   `workflow_dispatch`. Ничего не гоняется на push/PR. Минимальный guard:
   ```yaml
   on: [pull_request]
   jobs:
     check:
       runs-on: windows-latest
       steps:
         - uses: actions/checkout@v4
         - run: cargo test --manifest-path frontend/src-tauri/Cargo.toml --no-fail-fast
         - run: cargo clippy --manifest-path frontend/src-tauri/Cargo.toml -- -D warnings
         - run: cd frontend && npx tsc --noEmit
   ```

3. **`typecheck`/`test` npm-скрипты** — ✅ **СДЕЛАНО.** В `package.json`
   добавлены `typecheck`, `typecheck:tests`, `test`, `test:watch`, `test:ui`.

4. **GigaAM транскрипционный harness** — ✅ **СДЕЛАНО.** Тест
   `gigaam_engine::tests::test_transcribe_real_audio` грузит движок по
   явному пути (`new_with_models_dir`, без `AppHandle`) и печатает транскрипт.
   Рефакторинга `GigaamEngine` не потребовалось — нужный конструктор уже был.
   См. выше раздел «Интеграционный тест с реальным аудио (GigaAM)».


5. **Frontend-тесты** — ✅ **СДЕЛАНО.** vitest + `@testing-library/react` +
   jsdom, 57 тестов в 6 файлах. См. выше раздел «Frontend». `SidebarProvider`
   сознательно отложен — он тянет `useRecordingState`, `usePathname`,
   `useRouter` и множество `invoke`, его изолированный тест дорогой и хрупкий;
   `useTemplates` покрывает тот же Tauri-hook-паттерн и был приоритетнее.

## Что НЕ делать

- Не вызывать `clean_run_windows.bat` для рутинной разработки.
- Не переустанавливать бандл для проверки каждой правки — есть `tauri:dev`
  и портативный `.exe`.
- Не запускать GigaAM-тесты в CI без скачанных моделей (они тяжёлые,
  227MB). Только локально с `TEST_AUDIO_PATH`.
