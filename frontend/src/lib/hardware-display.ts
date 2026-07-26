// Pure display helpers for the hardware card. Extracted from
// SetupOverviewStep.tsx so they can be unit-tested without rendering the
// full onboarding tree (which pulls in useOnboarding, plugin-os, etc.).
import type { HardwareProfileInfo } from '@/types/hardware';

/**
 * Human-readable GPU label for the hardware card.
 * - "NVIDIA GeForce RTX 3060" when a marketing name was detected.
 * - "GPU (vulkan)" when a GPU type is known but the name is missing.
 * - "Не обнаружен" when no GPU.
 */
export function gpuLabel(hw: HardwareProfileInfo | null): string {
  if (!hw) return '—';
  if (hw.gpuName) return hw.gpuName;
  if (hw.hasGpu) return `GPU (${hw.gpuType})`;
  return 'Не обнаружен';
}

/**
 * Formatted VRAM string ("12 ГБ") or "—" when unknown.
 */
export function vramLabel(hw: HardwareProfileInfo | null): string {
  if (!hw || hw.gpuVramGb == null) return '—';
  return `${Math.round(hw.gpuVramGb)} ГБ`;
}

/**
 * Tailwind class bundle for the inference-mode badge. Green for GPU, gray
 * for CPU.
 */
export function inferenceBadgeClass(mode: string): string {
  return mode.startsWith('GPU')
    ? 'bg-green-100 text-green-700'
    : 'bg-gray-100 text-gray-600';
}

/**
 * True when the recommended inference mode is any GPU variant.
 */
export function isInferenceGpu(mode: string): boolean {
  return mode.startsWith('GPU');
}
