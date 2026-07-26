import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Info, Cpu, MemoryStick, Zap, MonitorDown } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { HardwareProfileInfo } from '@/types/hardware';

function StatusDot({ on }: { on: boolean }) {
  return (
    <span
      className={`inline-block h-2.5 w-2.5 rounded-full ${
        on ? 'bg-green-500' : 'bg-gray-300'
      }`}
      aria-label={on ? 'доступно' : 'недоступно'}
    />
  );
}

function HardwareRow({
  icon: Icon,
  label,
  value,
  dot,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: React.ReactNode;
  dot?: boolean;
}) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="flex items-center gap-2 text-sm text-gray-600">
        <Icon className="h-4 w-4 text-gray-400" />
        {label}
      </span>
      <span className="flex items-center gap-2 text-sm font-medium text-gray-900">
        {value}
        {dot !== undefined && <StatusDot on={dot} />}
      </span>
    </div>
  );
}

export function SetupOverviewStep() {
  const { goNext } = useOnboarding();
  const [isMac, setIsMac] = useState(false);
  const [hw, setHw] = useState<HardwareProfileInfo | null>(null);

  useEffect(() => {
    const checkPlatform = async () => {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
    // Hardware detection is best-effort: failure leaves `hw` null and the
    // card is simply not rendered.
    invoke<HardwareProfileInfo>('api_get_hardware_profile')
      .then(setHw)
      .catch((e) => console.warn('Hardware detection failed:', e));
  }, []);

  const steps = [
    {
      number: 1,
      type: 'transcription',
      title: 'Скачать движок транскрипции',
    },
    {
      number: 2,
      type: 'summarization',
      title: 'Скачать движок суммаризации',
    },
  ];

  const handleContinue = () => {
    goNext();
  };

  // GPU display strings.
  const gpuLabel = hw
    ? hw.gpuName
      ? hw.gpuName
      : hw.hasGpu
        ? `GPU (${hw.gpuType})`
        : 'Не обнаружен'
    : '—';
  const vramLabel =
    hw?.gpuVramGb != null ? `${Math.round(hw.gpuVramGb)} ГБ` : '—';
  const inferenceBadge = hw?.recommendedInferenceMode ?? '—';
  const isGpu = inferenceBadge.startsWith('GPU');

  return (
    <OnboardingContainer
      title="Обзор настройки"
      description="Для работы Meetly необходимо загрузить модели транскрипции и суммаризации."
      step={2}
      totalSteps={isMac ? 4 : 3}
    >
      <div className="flex flex-col items-center space-y-10">
        {/* Hardware card (read-only) */}
        {hw && (
          <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 p-4">
            <div className="mb-2 flex items-center gap-2">
              <Zap className="h-4 w-4 text-gray-700" />
              <h3 className="text-sm font-semibold text-gray-900">
                Обнаруженное оборудование
              </h3>
            </div>

            <HardwareRow
              icon={Cpu}
              label="Процессор (ядра)"
              value={hw.cpuCores}
            />
            <HardwareRow
              icon={MemoryStick}
              label="Оперативная память"
              value={`${hw.memoryGb} ГБ`}
            />
            <HardwareRow
              icon={MonitorDown}
              label="Видеокарта"
              value={gpuLabel}
              dot={hw.hasGpu}
            />
            {hw.hasGpu && (
              <HardwareRow
                icon={MemoryStick}
                label="Видеопамять (VRAM)"
                value={vramLabel}
              />
            )}

            {/* Inference mode + recommended model */}
            <div className="mt-3 border-t border-gray-100 pt-3">
              <div className="flex items-center justify-between py-1.5">
                <span className="text-sm text-gray-600">Инференс резюме</span>
                <span
                  className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                    isGpu
                      ? 'bg-green-100 text-green-700'
                      : 'bg-gray-100 text-gray-600'
                  }`}
                >
                  {inferenceBadge}
                </span>
              </div>
              <div className="flex items-center justify-between py-1.5">
                <span className="text-sm text-gray-600">
                  Рекомендованная модель
                </span>
                <span className="text-sm font-medium text-gray-900">
                  {hw.recommendedModel}
                </span>
              </div>
              <p className="mt-1 text-xs text-gray-500">
                Система автоматически подобрала оптимальную модель под ваше
                оборудование. Настройки можно изменить позже.
              </p>
            </div>
          </div>
        )}

        {/* Steps Card */}
        <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 p-4">
          <div className="space-y-4">
            {steps.map((step, idx) => {
              return (
                <div
                  key={step.number}
                  className={`flex items-start gap-4 p-1`}
                >
                  <div className="flex-1 ml-1">
                    <h3 className="font-medium text-gray-900 flex items-center gap-2">
                      Шаг {step.number} :  {step.title}

                      {step.type === "summarization" && (
                        <TooltipProvider>
                        <Tooltip>
                            <TooltipTrigger asChild>
                            <button className="text-gray-400 hover:text-gray-600">
                                <Info className="w-4 h-4" />
                            </button>
                            </TooltipTrigger>
                            <TooltipContent className="max-w-xs text-sm">
                            Также в настройках можно выбрать внешние AI-провайдеры
                            для генерации резюме: OpenAI, Claude или Ollama.
                            </TooltipContent>
                        </Tooltip>
                        </TooltipProvider>
                      )}
                    </h3>
                  </div>
                </div>
              );
            })}
          </div>
        </div>


        {/* CTA Section */}
        <div className="w-full max-w-xs space-y-4">
          <Button
            onClick={handleContinue}
            className="w-full h-11 bg-gray-900 hover:bg-gray-800 text-white"
          >
            Поехали
          </Button>
          <div className="text-center">
            <a
              href="https://github.com/Zackriya-Solutions/meeting-minutes"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-gray-600 hover:underline"
            >
              Сообщить о проблемах на GitHub
            </a>
          </div>
        </div>
      </div>
    </OnboardingContainer>
  );
}
