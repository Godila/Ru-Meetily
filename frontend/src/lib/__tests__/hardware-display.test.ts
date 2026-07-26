import { describe, it, expect } from 'vitest';
import {
  gpuLabel,
  vramLabel,
  inferenceBadgeClass,
  isInferenceGpu,
} from '@/lib/hardware-display';
import type { HardwareProfileInfo } from '@/types/hardware';

function hw(overrides: Partial<HardwareProfileInfo> = {}): HardwareProfileInfo {
  return {
    cpuCores: 8,
    memoryGb: 16,
    gpuType: 'vulkan',
    gpuName: null,
    gpuVramGb: null,
    vulkanAvailable: true,
    performanceTier: 'high',
    recommendedModel: 'qwen3.5:4b',
    recommendedInferenceMode: 'GPU (Vulkan)',
    hasGpu: true,
    ...overrides,
  };
}

describe('gpuLabel', () => {
  it('returns the detected marketing name when present', () => {
    expect(gpuLabel(hw({ gpuName: 'NVIDIA GeForce RTX 3060' }))).toBe(
      'NVIDIA GeForce RTX 3060'
    );
  });

  it('falls back to "GPU (<type>)" when name is missing but a GPU exists', () => {
    expect(gpuLabel(hw({ gpuName: null, gpuType: 'vulkan', hasGpu: true }))).toBe(
      'GPU (vulkan)'
    );
  });

  it('returns "Не обнаружен" when hasGpu is false', () => {
    expect(gpuLabel(hw({ hasGpu: false, gpuType: 'none', gpuName: null }))).toBe(
      'Не обнаружен'
    );
  });

  it('returns "—" when the profile is null (detection failed)', () => {
    expect(gpuLabel(null)).toBe('—');
  });
});

describe('vramLabel', () => {
  it('rounds VRAM to whole GB', () => {
    expect(vramLabel(hw({ gpuVramGb: 11.97 }))).toBe('12 ГБ');
  });

  it('returns "—" when VRAM is null (unified memory / unknown)', () => {
    expect(vramLabel(hw({ gpuVramGb: null }))).toBe('—');
  });

  it('returns "—" when the profile is null', () => {
    expect(vramLabel(null)).toBe('—');
  });
});

describe('inferenceBadgeClass', () => {
  it('uses the green bundle for GPU modes', () => {
    expect(inferenceBadgeClass('GPU (Vulkan)')).toBe('bg-green-100 text-green-700');
    expect(inferenceBadgeClass('GPU (Metal)')).toBe('bg-green-100 text-green-700');
  });

  it('uses the gray bundle for CPU mode', () => {
    expect(inferenceBadgeClass('CPU')).toBe('bg-gray-100 text-gray-600');
  });
});

describe('isInferenceGpu', () => {
  it('is true for any GPU mode', () => {
    expect(isInferenceGpu('GPU (Vulkan)')).toBe(true);
    expect(isInferenceGpu('GPU (CUDA)')).toBe(true);
  });

  it('is false for CPU and unknown modes', () => {
    expect(isInferenceGpu('CPU')).toBe(false);
    expect(isInferenceGpu('—')).toBe(false);
  });
});
