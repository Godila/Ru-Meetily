'use client';

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { ModelConfig, ModelSettingsModal } from '@/components/ModelSettingsModal';
import { SummaryLanguageSettings } from '@/components/SummaryLanguageSettings';
import { Switch } from './ui/switch';
import { useConfig } from '@/contexts/ConfigContext';
import type { HardwareProfileInfo } from '@/types/hardware';

interface SummaryModelSettingsProps {
  refetchTrigger?: number; // Change this to trigger refetch
}

export function SummaryModelSettings({ refetchTrigger }: SummaryModelSettingsProps) {
  const [modelConfig, setModelConfig] = useState<ModelConfig>({
    provider: 'ollama',
    model: 'llama3.2:latest',
    whisperModel: 'large-v3',
    apiKey: null,
    ollamaEndpoint: null
  });

  const [useGpu, setUseGpu] = useState<boolean>(true);
  const [hasGpu, setHasGpu] = useState<boolean>(false);

  const { isAutoSummary, toggleIsAutoSummary } = useConfig();

  // Reusable fetch function
  const fetchModelConfig = useCallback(async () => {
    try {
      const data = await invoke('api_get_model_config') as any;
      if (data && data.provider !== null) {
        // Fetch API key if not included and provider requires it
        if (data.provider !== 'ollama' && data.provider !== 'builtin-ai' && !data.apiKey) {
          try {
            const apiKeyData = await invoke('api_get_api_key', {
              provider: data.provider
            }) as string;
            data.apiKey = apiKeyData;
          } catch (err) {
            console.error('Failed to fetch API key:', err);
          }
        }
        // Fetch Custom OpenAI config if that's the active provider
        if (data.provider === 'custom-openai') {
          try {
            const customConfig = (await invoke('api_get_custom_openai_config')) as any;
            if (customConfig) {
              data.customOpenAIDisplayName = customConfig.displayName || null;
              data.customOpenAIEndpoint = customConfig.endpoint || null;
              data.customOpenAIModel = customConfig.model || null;
              data.customOpenAIApiKey = customConfig.apiKey || null;
              data.maxTokens = customConfig.maxTokens || null;
              data.temperature = customConfig.temperature || null;
              data.topP = customConfig.topP || null;
              // For custom-openai, model field should match customOpenAIModel
              data.model = customConfig.model || data.model;
            }
          } catch (err) {
            console.error('Failed to fetch custom OpenAI config:', err);
          }
        }
        setModelConfig(data);
        // useGpu is optional in the response; default to true when absent.
        if (data.useGpu !== undefined && data.useGpu !== null) {
          setUseGpu(data.useGpu);
        }
      }
    } catch (error) {
      console.error('Failed to load model settings', error);
      toast.error('Failed to load model settings');
    }
  }, []);

  // Detect hardware once on mount to drive the GPU-toggle availability.
  useEffect(() => {
    invoke<HardwareProfileInfo>('api_get_hardware_profile')
      .then((info) => setHasGpu(info.hasGpu))
      .catch((e) => console.warn('Hardware detection failed:', e));
  }, []);

  // Fetch on mount
  useEffect(() => {
    fetchModelConfig();
  }, [fetchModelConfig]);

  // Refetch when trigger changes (optional external control)
  useEffect(() => {
    if (refetchTrigger !== undefined && refetchTrigger > 0) {
      fetchModelConfig();
    }
  }, [refetchTrigger, fetchModelConfig]);

  // Listen for model config updates from other components
  useEffect(() => {
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<ModelConfig>('model-config-updated', (event) => {
        console.log('SummaryModelSettings received model-config-updated event:', event.payload);
        setModelConfig(event.payload);
      });

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then(fn => cleanup = fn);

    return () => {
      cleanup?.();
    };
  }, []);

  // Save handler
  const handleSaveModelConfig = async (config: ModelConfig) => {
    try {
      await invoke('api_save_model_config', {
        provider: config.provider,
        model: config.model,
        whisperModel: config.whisperModel,
        apiKey: config.apiKey,
        ollamaEndpoint: config.ollamaEndpoint,
      });

      setModelConfig(config);

      // Emit event to sync other components
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', config);

      toast.success('Model settings saved successfully');
    } catch (error) {
      console.error('Error saving model config:', error);
      toast.error('Failed to save model settings');
    }
  };

  const toggleGpu = async (enabled: boolean) => {
    const prev = useGpu;
    setUseGpu(enabled); // optimistic
    try {
      await invoke('api_set_use_gpu', { useGpu: enabled });
      toast.success(enabled ? 'GPU включён' : 'Принудительный CPU');
    } catch (e) {
      setUseGpu(prev); // rollback
      console.error('Failed to set use_gpu', e);
      toast.error('Не удалось изменить настройку GPU');
    }
  };

  return (
    <div className='flex flex-col gap-4'>
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Авто-резюме</h3>
            <p className="text-sm text-gray-600">Автоматически генерировать резюме после завершения встречи (остановки записи)</p>
          </div>
          <Switch checked={isAutoSummary} onCheckedChange={toggleIsAutoSummary} />
        </div>
      </div>

      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between">
          <div className="pr-4">
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              Быстрая генерация на видеокарте
            </h3>
            <p className="text-sm text-gray-600">
              {hasGpu
                ? 'Резюме генерируется быстрее за счёт видеокарты. Величина ускорения зависит от вашей видеокарты. Отключайте, если резюме перестало генерироваться или появились ошибки.'
                : 'Видеокарта не обнаружена — резюме генерируется на процессоре. Это медленнее, но работает на любом компьютере.'}
            </p>
          </div>
          <Switch
            checked={useGpu && hasGpu}
            onCheckedChange={toggleGpu}
            disabled={!hasGpu}
          />
        </div>
      </div>

      <SummaryLanguageSettings />

      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <h3 className="text-lg font-semibold mb-4">Настройка модели резюме</h3>
        <p className="text-sm text-gray-600 mb-6">
          Настройте AI-модель, используемую для генерации резюме встреч.
        </p>

        <ModelSettingsModal
          modelConfig={modelConfig}
          setModelConfig={setModelConfig}
          onSave={handleSaveModelConfig}
          skipInitialFetch={true}
        />
      </div>
    </div>
  );
}
