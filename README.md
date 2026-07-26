<div align="center">
    <h1>Convoic</h1>
    <p><strong>Голос ваших встреч, превращённый в смысл.</strong></p>
    <p>Локальный AI-ассистент для встреч с распознаванием русской речи на базе GigaAM</p>
    <p>
        <img src="https://img.shields.io/badge/License-MIT-blue" alt="License">
        <img src="https://img.shields.io/badge/Platform-Windows-white" alt="Platform">
        <img src="https://img.shields.io/badge/STT-GigaAM--v3_RNN--T-orange" alt="STT Model">
        <img src="https://img.shields.io/badge/version-0.5.0-green" alt="Version">
    </p>
</div>

---

**Convoic** = *convo* (разговор) + *voice* (голос). Локальный AI-ассистент для встреч, который слушает, расшифровывает и саммарирует — без отправки данных в облако.

Проект основан на форке [Meetily](https://github.com/Zackriya-Solutions/meetily) и адаптирован для русского языка: движок распознавания речи (Parakeet → **GigaAM-v3 E2E RNN-T** от Сбера) заменён для значительно более точного распознавания русской речи с пунктуацией и правильным регистром. Полностью локальная обработка — данные никогда не покидают ваш компьютер.

## Ключевые особенности

- **🎯 GigaAM-v3 RNN-T вместо Parakeet.** Транскрипция на русском с пунктуацией и заглавными буквами. Модель Sber GigaAM через ONNX Runtime (Rust), без Python.
- **🇷🇺 Полная русская локализация интерфейса.** Все экраны, онбординг, настройки, диалоги, трей-меню переведены на русский.
- **🪟 Сборка под Windows (RU).** Локаль сборки изменена с en-US на RU, NSIS-установщик на русском языке.
- **🚫 Whisper удалён из сборки.** Заявленная функциональность Whisper отключена (заглушка), фокус — на GigaAM.
- **📊 Аналитика отключена по умолчанию.** Сбор usage-аналитики выключен, тумблер убран из UI.

## Возможности

- **Локальная транскрипция в реальном времени** через GigaAM-v3 RNN-T (с пунктуацией)
- **Импорт аудиофайлов** (MP4, M4A, WAV, MP3, FLAC, OGG, AAC, MKV, WebM, WMA) — drag-and-drop или через диалог
- **AI-резюме встреч** через Ollama (локально), Claude, Groq, OpenRouter или любой OpenAI-совместимый эндпоинт
- **Запись системного звука и микрофона** одновременно (WASAPI loopback на Windows)
- **Фоновая генерация саммари** — переживает навигацию между экранами, индикатор в сайдбаре
- **Приватность по умолчанию** — все модели и данные хранятся локально

## Установка (Windows)

1. Скачайте последний `Convoic_0.5.0_x64-setup.exe` со страницы [Releases](https://github.com/Godila/convoic/releases/latest)
2. Запустите установщик
3. При первом запуске приложение скачает модель GigaAM (~227 МБ) — это займёт пару минут

> Модель хранится в `%APPDATA%\com.convoic.app\models\gigaam\gigaam-v3-rnnt-int8\`

> ⚠️ **Если вы обновляетесь с Ru-Meetily 0.4.x:** Convoic — это ребрендинг с новым identifier (`com.convoic.app` вместо `com.meetily.ai`), поэтому он стартует с чистого листа. Старое приложение Ru-Meetily останется установленным, его можно удалить вручную вместе с папкой `%APPDATA%\com.meetily.ai\`. Перенос встреч/моделей не выполняется автоматически.

## Использование

### Запись встречи
1. Выберите устройства ввода (микрофон + системный звук) в настройках
2. Нажмите кнопку записи — транскрипция появится в реальном времени
3. После остановки генерируется AI-резюме (если настроен провайдер)

### Импорт аудио
Перетащите аудиофайл в окно приложения или используйте кнопку импорта. Поддерживаются все основные форматы.

### AI-резюме
Настройте провайдера в Настройки → Резюме:
- **Ollama** (рекомендуется, локально) — установите [Ollama](https://ollama.ai) и скачайте модель (например, `qwen3.5:4b`)
- **Claude / Groq / OpenRouter** — введите API-ключ

## Архитектура

Convoic — это приложение на [Tauri](https://tauri.app/) (v2):
- **Backend:** Rust (`frontend/src-tauri/`) — захват аудио, STT через ONNX Runtime, БД SQLite
- **Frontend:** Next.js (`frontend/`) — UI на React/TypeScript

### Движок распознавания речи

Используется **GigaAM-v3 E2E RNN-T** (int8, ~227 МБ) — конформер с трансдьюсером, выдающий текст с пунктуацией:
- 3 ONNX-модели: encoder (Conformer), decoder (LSTM-предиктор), joint
- Mel-спектрограмма вычисляется в Rust (порт `onnx-asr` препроцессора)
- Greedy RNN-T декодирование (до 3 токенов на фрейм, 40мс на фрейм)
- BPE-словарь на 1025 токенов

Детали реализации см. в `frontend/src-tauri/src/gigaam_engine/`.

## Сборка из исходников

### Требования
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+ и [pnpm](https://pnpm.io/)
- [LLVM](https://github.com/llvm/llvm-project/releases) (для bindgen)
- Visual Studio Build Tools (MSVC)

### Шаги
```bash
git clone https://github.com/Godila/convoic
cd convoic/frontend
pnpm install
pnpm tauri build
```

Готовый установщик: `target/release/bundle/nsis/Convoic_0.5.0_x64-setup.exe`

Подробности сборки на других ОС — в [docs/BUILDING.md](docs/BUILDING.md).

## Для разработчиков

Запуск в dev-режиме:
```bash
cd frontend
pnpm tauri:dev:cpu
```

GPU-ускорение (опционально): `pnpm tauri:dev:cuda` / `pnpm tauri:dev:vulkan`.

## Лицензия

MIT License — см. [LICENSE](LICENSE).

## Благодарности

- [Meetily](https://github.com/Zackriya-Solutions/meetily) (Zackriya-Solutions) — оригинальный проект, на котором основан этот форк
- [GigaAM](https://github.com/salute-developers/GigaAM) (Сбер) — модель распознавания русской речи
- [istupakov/gigaam-v3-onnx](https://huggingface.co/istupakov/gigaam-v3-onnx) — ONNX-конверсия модели GigaAM-v3
- [onnx-asr](https://github.com/istupakov/onnx-asr) (istupakov) — reference-реализация инференса, по которой портировался движок на Rust

## История бренда

Convoic сменил имя с **Ru-Meetily** в версии 0.5.0 (2026-07-25). Прошлые версии выпускались как Ru-Meetily. См. [CHANGELOG.md](CHANGELOG.md) для деталей.
