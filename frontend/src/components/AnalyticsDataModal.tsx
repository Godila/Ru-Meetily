'use client';

import React from 'react';
import { X, Info, Shield } from 'lucide-react';

interface AnalyticsDataModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirmDisable: () => void;
}

export default function AnalyticsDataModal({ isOpen, onClose, onConfirmDisable }: AnalyticsDataModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-200">
          <div className="flex items-center gap-3">
            <Shield className="w-6 h-6 text-blue-600" />
            <h2 className="text-xl font-semibold text-gray-900">Что собирает аналитика</h2>
          </div>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6">
          {/* Privacy Notice */}
          <div className="bg-green-50 border border-green-200 rounded-lg p-4">
            <div className="flex items-start gap-3">
              <Info className="w-5 h-5 text-green-600 mt-0.5 flex-shrink-0" />
              <div className="text-sm text-green-800">
                <p className="font-semibold mb-1">Ваша приватность защищена</p>
                <p>Аналитика выключена по умолчанию. Если включить её, мы собираем <strong>только анонимные данные об использовании</strong>. Никакого содержимого встреч, имён, путей к файлам или личной информации.</p>
              </div>
            </div>
          </div>

          {/* Data Categories */}
          <div className="space-y-4">
            <h3 className="text-lg font-semibold text-gray-900">Данные, которые мы собираем при включении:</h3>

            {/* Model Preferences */}
            <div className="border border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">1. Предпочтения по моделям</h4>
              <ul className="text-sm text-gray-700 space-y-1 ml-4">
                <li>• Модель транскрипции (напр., «Whisper large-v3», «Parakeet»)</li>
                <li>• Модель саммари (напр., «Llama 3.2», «Claude Sonnet»)</li>
                <li>• Провайдер модели (напр., «Локально», «Ollama», «OpenRouter»)</li>
              </ul>
              <p className="text-xs text-gray-500 mt-2 italic">Помогает понять, какие модели предпочитают пользователи</p>
            </div>

            {/* Meeting Metrics */}
            <div className="border border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">2. Анонимная статистика встреч</h4>
              <ul className="text-sm text-gray-700 space-y-1 ml-4">
                <li>• Длительность записи (напр., «125 секунд»)</li>
                <li>• Длительность паузы (напр., «5 секунд»)</li>
                <li>• Количество фрагментов транскрипции</li>
                <li>• Количество обработанных аудиофрагментов</li>
              </ul>
              <p className="text-xs text-gray-500 mt-2 italic">Помогает оптимизировать производительность и понимать паттерны использования</p>
            </div>

            {/* Device Types */}
            <div className="border border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">3. Типы устройств (не названия)</h4>
              <ul className="text-sm text-gray-700 space-y-1 ml-4">
                <li>• Тип микрофона: «Bluetooth», «Проводное» или «Неизвестно»</li>
                <li>• Тип системного аудио: «Bluetooth», «Проводное» или «Неизвестно»</li>
              </ul>
              <p className="text-xs text-gray-500 mt-2 italic">Помогает улучшить совместимость, а НЕ названия реальных устройств</p>
            </div>

            {/* Usage Patterns */}
            <div className="border border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">4. Паттерны использования</h4>
              <ul className="text-sm text-gray-700 space-y-1 ml-4">
                <li>• События запуска/остановки приложения</li>
                <li>• Длительность сессии</li>
                <li>• Использование функций (напр., «настройки изменены»)</li>
                <li>• Возникновение ошибок (помогает исправлять баги)</li>
              </ul>
              <p className="text-xs text-gray-500 mt-2 italic">Помогает улучшить пользовательский опыт</p>
            </div>

            {/* Platform Info */}
            <div className="border border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">5. Информация о платформе</h4>
              <ul className="text-sm text-gray-700 space-y-1 ml-4">
                <li>• Операционная система (напр., «macOS», «Windows»)</li>
                <li>• Версия приложения (автоматически включается во все события)</li>
                <li>• Архитектура (напр., «x86_64», «aarch64»)</li>
              </ul>
              <p className="text-xs text-gray-500 mt-2 italic">Помогает приоритизировать поддержку платформ</p>
            </div>
          </div>

          {/* What We DON'T Collect */}
          <div className="bg-red-50 border border-red-200 rounded-lg p-4">
            <h4 className="font-semibold text-red-900 mb-2">Что мы НЕ собираем:</h4>
            <ul className="text-sm text-red-800 space-y-1 ml-4">
              <li>• ❌ Названия встреч</li>
              <li>• ❌ Имена файлов, пути к файлам или папки встреч</li>
              <li>• ❌ Транскрипции встреч или их содержимое</li>
              <li>• ❌ Аудиозаписи</li>
              <li>• ❌ Названия устройств (только типы: Bluetooth/Проводное)</li>
              <li>• ❌ Личную информацию</li>
              <li>• ❌ Любые идентифицирующие данные</li>
            </ul>
          </div>

          {/* Example Event */}
          <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Пример события:</h4>
            <pre className="text-xs text-gray-700 overflow-x-auto">
              {`{
  "event": "meeting_ended",
  "app_version": "0.5.2",
  "transcription_provider": "gigaam",
  "transcription_model": "gigaam-v3-rnnt-int8",
  "summary_provider": "ollama",
  "summary_model": "llama3.2:latest",
  "total_duration_seconds": "125.5",
  "microphone_device_type": "Wired",
  "system_audio_device_type": "Bluetooth",
  "chunks_processed": "150",
  "had_fatal_error": "false"
}`}
            </pre>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between gap-4 p-6 border-t border-gray-200 bg-gray-50">
          <button
            onClick={onClose}
            className="px-4 py-2 text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 transition-colors"
          >
            Оставить включённой
          </button>
          <button
            onClick={onConfirmDisable}
            className="px-4 py-2 text-white bg-red-600 rounded-md hover:bg-red-700 transition-colors"
          >
            Отключить аналитику
          </button>
        </div>
      </div>
    </div>
  );
}
