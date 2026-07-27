'use client';

import React, { useState } from 'react';
import { Cpu, Cloud, Clock, ChevronLeft, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import {
  ModelConfig,
  ModelSettingsModal,
} from '@/components/ModelSettingsModal';
import type {
  SummaryProviderDecision,
  CloudProviderId,
} from '@/types/provider-decision';
import { CLOUD_PROVIDERS } from '@/types/provider-decision';

type Choice = 'local' | 'cloud' | 'skip' | null;

/**
 * Step 3 of onboarding: the user picks how summaries will be generated.
 *
 * - Local: download a built-in Qwen model (~1–3 GiB). Auto-recommended from RAM,
 *   no provider/model picker in onboarding (lives in Settings afterwards).
 * - Cloud: reuse the full ModelSettingsModal inline (provider dropdown, API key,
 *   "Проверить подключение"). Caila is preselected as the Russian-market default.
 * - Skip: defer provider selection. The app will prompt again on first summary
 *   generation (lazy gate via ProviderSetupGateContext).
 */
export function ProviderChoiceStep() {
  const {
    goNext,
    recommendedSummaryModel,
    setProviderDecision,
    setSummaryModelDownloaded,
  } = useOnboarding();
  const [choice, setChoice] = useState<Choice>(null);
  const [isMac, setIsMac] = useState(false);

  // Cloud-form draft state. We seed provider=caila (Russian-market default)
  // and let the user change it via the modal's Select. Model is left blank:
  // the modal's auto-fetch effect loads Caila models once a key is entered.
  const [cloudConfig, setCloudConfig] = useState<ModelConfig>({
    provider: 'caila',
    model: '',
    whisperModel: '',
    apiKey: null,
    ollamaEndpoint: null,
  });

  React.useEffect(() => {
    const checkPlatform = async () => {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch {
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  const handleChoose = (next: Choice) => {
    setChoice(next);
    if (next === 'local' || next === 'skip') {
      // Local and Skip are final — commit the decision immediately and advance.
      const decision: SummaryProviderDecision =
        next === 'local'
          ? { kind: 'local', model: recommendedSummaryModel || 'qwen3.5:4b' }
          : { kind: 'deferred', reason: 'user_skipped' };
      setProviderDecision(decision);
      // Reset summary download flag for non-local branches so the downstream
      // DownloadProgressStep does not think a local model is already present.
      if (next === 'skip') setSummaryModelDownloaded(false);
      goNext();
    }
    // For 'cloud' we stay on this step and reveal the inline settings form.
  };

  const handleCloudSave = (cfg: ModelConfig) => {
    // Only cloud providers reach this branch (the inline modal is only shown
    // when choice==='cloud'). The cast is safe: the modal's Select only lists
    // the providers in CLOUD_PROVIDERS (Caila/OpenAI/Claude/OpenRouter/Custom).
    if (!CLOUD_PROVIDERS.includes(cfg.provider as CloudProviderId)) {
      console.warn(
        '[ProviderChoiceStep] non-cloud provider reached cloud save:',
        cfg.provider
      );
      return;
    }
    const decision: SummaryProviderDecision = {
      kind: 'cloud',
      provider: cfg.provider as CloudProviderId,
      apiKey: cfg.apiKey ?? '',
      model: cfg.model,
    };
    setProviderDecision(decision);
    setCloudConfig(cfg);
    goNext();
  };

  // The card list is hidden once the user enters the cloud form so the form
  // gets the full width.
  if (choice === 'cloud') {
    return (
      <OnboardingContainer
        title="Облачный провайдер"
        description="Введите API-ключ и выберите модель. Caila рекомендуется для российских пользователей."
        step={3}
        totalSteps={isMac ? 5 : 4}
      >
        <div className="flex flex-col items-center space-y-4">
          <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 shadow-sm p-4">
            <ModelSettingsModal
              modelConfig={cloudConfig}
              setModelConfig={setCloudConfig}
              onSave={handleCloudSave}
              skipInitialFetch
              layout="inline"
            />
          </div>
          <Button
            variant="ghost"
            className="text-gray-500 hover:text-gray-700"
            onClick={() => setChoice(null)}
          >
            <ChevronLeft className="w-4 h-4 mr-1" />
            Назад к выбору
          </Button>
        </div>
      </OnboardingContainer>
    );
  }

  const cards: {
    id: 'local' | 'cloud' | 'skip';
    icon: React.ComponentType<{ className?: string }>;
    title: string;
    badge?: string;
    desc: string;
    meta: string;
  }[] = [
    {
      id: 'local',
      icon: Cpu,
      title: 'Локальная модель',
      desc: 'Скачивается встроенная модель Qwen и работает офлайн. Все данные остаются на устройстве.',
      meta: recommendedSummaryModel
        ? `Рекомендуется: ${recommendedSummaryModel}`
        : 'Размер зависит от модели',
    },
    {
      id: 'cloud',
      icon: Cloud,
      title: 'Облачный провайдер',
      badge: 'Рекомендуется для РФ',
      desc: 'Caila, OpenAI, Claude или OpenRouter. Ничего не скачивается — нужен только API-ключ.',
      meta: 'Caila выбрана по умолчанию',
    },
    {
      id: 'skip',
      icon: Clock,
      title: 'Пропустить',
      desc: 'Настроить позже. При первой попытке сгенерировать резюме приложение спросит провайдера.',
      meta: 'Самый быстрый путь',
    },
  ];

  return (
    <OnboardingContainer
      title="Как генерировать резюме?"
      description="Выберите способ генерации саммари. Модель распознавания речи скачивается в любом случае."
      step={3}
      totalSteps={isMac ? 5 : 4}
    >
      <div className="flex flex-col items-center space-y-3">
        <div className="w-full max-w-md space-y-3">
          {cards.map((card) => {
            const Icon = card.icon;
            const selected = choice === card.id;
            return (
              <button
                key={card.id}
                onClick={() => handleChoose(card.id)}
                className={`w-full text-left bg-white rounded-lg border p-4 transition-all ${
                  selected
                    ? 'border-gray-900 ring-1 ring-gray-900'
                    : 'border-gray-200 hover:border-gray-400 hover:shadow-sm'
                }`}
              >
                <div className="flex items-start gap-3">
                  <div className="flex-shrink-0 mt-0.5 w-9 h-9 rounded-full bg-gray-100 flex items-center justify-center">
                    <Icon className="w-5 h-5 text-gray-700" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <h3 className="text-sm font-semibold text-gray-900">
                        {card.title}
                      </h3>
                      {card.badge && (
                        <span className="text-[10px] uppercase tracking-wide font-medium text-blue-700 bg-blue-50 px-1.5 py-0.5 rounded">
                          {card.badge}
                        </span>
                      )}
                      {selected && (
                        <Check className="w-4 h-4 text-gray-900 ml-auto" />
                      )}
                    </div>
                    <p className="mt-1 text-xs text-gray-600 leading-relaxed">
                      {card.desc}
                    </p>
                    <p className="mt-2 text-[11px] text-gray-400">{card.meta}</p>
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </div>
    </OnboardingContainer>
  );
}
