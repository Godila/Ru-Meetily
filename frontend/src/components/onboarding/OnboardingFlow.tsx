import React, { useEffect } from 'react';
import { useOnboarding } from '@/contexts/OnboardingContext';
import {
  WelcomeStep,
  PermissionsStep,
  DownloadProgressStep,
  SetupOverviewStep,
  ProviderChoiceStep,
} from './steps';

interface OnboardingFlowProps {
  onComplete: () => void;
}

export function OnboardingFlow({ onComplete }: OnboardingFlowProps) {
  const { currentStep } = useOnboarding();
  const [isMac, setIsMac] = React.useState(false);

  useEffect(() => {
    // Check if running on macOS
    const checkPlatform = async () => {
      try {
        // Dynamic import to avoid SSR issues if any
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        console.error('Failed to detect platform:', e);
        // Fallback
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  // 5-Step Onboarding Flow (optional LLM provider):
  // Step 1: Welcome - Introduce Convoic features
  // Step 2: Setup Overview - Database initialization + detected hardware
  // Step 3: Provider Choice - Local model | Cloud provider | Skip (NEW)
  // Step 4: Download Progress - GigaAM (always) + Qwen (only if Local chosen)
  // Step 5: Permissions - Request mic + system audio (macOS only)

  return (
    <div className="onboarding-flow">
      {currentStep === 1 && <WelcomeStep />}
      {currentStep === 2 && <SetupOverviewStep />}
      {currentStep === 3 && <ProviderChoiceStep />}
      {currentStep === 4 && <DownloadProgressStep />}
      {currentStep === 5 && isMac && <PermissionsStep />}
    </div>
  );
}
