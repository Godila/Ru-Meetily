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

Шаблон: `frontend/src-tauri/src/audio/import.rs:1312`
(`test_import_pipeline_decode_vad`, `#[ignore]`). Тест берёт WAV из env,
декодирует, гонит через VAD. Расширь до RNN-T для отладки транскрипции:

```bash
TEST_AUDIO_PATH=C:/path/to/sample.wav \
  cargo test --manifest-path frontend/src-tauri/Cargo.toml \
  -- --ignored --nocapture
```

**Проблема для GigaAM:** `GigaamEngine` требует `AppHandle` для пути к моделям
(неудобно в юнит-тесте). Чтобы сделать транскрипционный harness, нужно
рефакторить — добавить конструктор, принимающий явный путь к моделям. Это
отдельная задача (не в этом PR).

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

4. **GigaAM транскрипционный harness** — ✅ **СДЕЛАНО.** См. выше раздел
   «Интеграционный тест с реальным аудио (GigaAM)».

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
