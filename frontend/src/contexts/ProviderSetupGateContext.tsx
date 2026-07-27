'use client';

/**
 * Global host for the LLM provider settings dialog.
 *
 * Before this context existed, `ModelSettingsModal` was opened from three
 * unrelated places:
 *   1. Sidebar — local `useState` + `window.openSettings` (tray hook)
 *   2. meeting-details — `openModelSettingsRef` registered by
 *      `SummaryGeneratorButtonGroup` and called from `useSummaryGeneration`
 *      when the backend reports a missing model.
 *   3. Settings tab — always-mounted inline.
 *
 * The onboarding lazy-gate (user pressed "Generate summary" after picking the
 * "Skip" branch) needs the same dialog from the summary hook, but without the
 * page-local ref machinery. This provider is the single source of truth for the
 * dialog's open state, exposes `openSettingsModal(reason)` to any consumer, and
 * keeps backward compatibility with the Rust tray by re-publishing
 * `window.openSettings`.
 *
 * The Settings tab keeps its own always-mounted inline instance because that's
 * a different layout ("inline", not "dialog"); this provider only owns the
 * floating dialog.
 *
 * `ModelSettingsModal` reads its config from `ConfigContext` when one is
 * available (see ModelSettingsModal.tsx:137), and `ConfigContext` is mounted
 * globally above this provider in `ClientLayout`. We therefore do NOT duplicate
 * the config here — we only own the dialog visibility and forward `onSave` to
 * the context's persister.
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';
import {
  ModelConfig,
  ModelSettingsModal,
} from '@/components/ModelSettingsModal';
import { useConfig } from '@/contexts/ConfigContext';
import { invoke } from '@tauri-apps/api/core';
import { VisuallyHidden } from '@/components/ui/visually-hidden';

export type ProviderSetupReason =
  | 'lazy_gate_summary_required'
  | 'user_action_sidebar'
  | 'user_action_tray'
  | 'post_onboarding_change';

interface ProviderSetupGateContextValue {
  /** Opens the global model-settings dialog. Idempotent. */
  openSettingsModal: (reason?: ProviderSetupReason) => void;
  /** Closes the dialog. */
  closeSettingsModal: () => void;
  /** Whether the dialog is currently open. */
  isOpen: boolean;
}

const ProviderSetupGateContext = createContext<
  ProviderSetupGateContextValue | undefined
>(undefined);

export function ProviderSetupGateProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const { modelConfig, setModelConfig } = useConfig();
  const [isOpen, setIsOpen] = useState(false);
  const lastReasonRef = useRef<ProviderSetupReason | null>(null);

  const openSettingsModal = useCallback(
    (reason: ProviderSetupReason = 'user_action_sidebar') => {
      lastReasonRef.current = reason;
      setIsOpen(true);
    },
    []
  );

  const closeSettingsModal = useCallback(() => {
    setIsOpen(false);
  }, []);

  // Backward-compat: Rust tray menu calls window.openSettings(). Re-publish it
  // here so we have a single owner regardless of which page is mounted.
  useEffect(() => {
    (window as unknown as { openSettings?: () => void }).openSettings = () =>
      openSettingsModal('user_action_tray');
    return () => {
      delete (window as unknown as { openSettings?: () => void }).openSettings;
    };
  }, [openSettingsModal]);

  const handleSave = useCallback(
    async (next: ModelConfig) => {
      // Persist to the DB, then mirror into the global ConfigContext state so
      // every consumer sees the update immediately. This mirrors the flow used
      // by Sidebar/index.tsx:185-196 and meeting-details/page-content.tsx.
      try {
        await invoke('api_save_model_config', {
          provider: next.provider,
          model: next.model,
          whisperModel: next.whisperModel,
          apiKey: next.apiKey,
          ollamaEndpoint: next.ollamaEndpoint,
        });
        setModelConfig(next);
      } catch (err) {
        console.error('[ProviderSetupGate] save failed:', err);
      }
    },
    [setModelConfig]
  );

  const value = useMemo<ProviderSetupGateContextValue>(
    () => ({ openSettingsModal, closeSettingsModal, isOpen }),
    [openSettingsModal, closeSettingsModal, isOpen]
  );

  return (
    <ProviderSetupGateContext.Provider value={value}>
      {children}
      <Dialog open={isOpen} onOpenChange={setIsOpen}>
        <DialogContent className="sm:max-w-2xl max-h-[90vh] overflow-y-auto">
          {/* Visually hidden title keeps the dialog accessible (Radix requires
              an accessible name) without duplicating the modal's own header. */}
          <VisuallyHidden>
            <DialogTitle>Настройки модели</DialogTitle>
          </VisuallyHidden>
          {/* ModelSettingsModal pulls config from ConfigContext on its own;
              the props here are only a fallback for when no context exists. */}
          <ModelSettingsModal
            modelConfig={modelConfig}
            setModelConfig={setModelConfig}
            onSave={handleSave}
            layout="dialog"
          />
        </DialogContent>
      </Dialog>
    </ProviderSetupGateContext.Provider>
  );
}

export function useProviderSetupGate(): ProviderSetupGateContextValue {
  const ctx = useContext(ProviderSetupGateContext);
  if (!ctx) {
    throw new Error(
      'useProviderSetupGate must be used within a ProviderSetupGateProvider'
    );
  }
  return ctx;
}
