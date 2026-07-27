import { describe, it, expect } from 'vitest';
import {
  getDownloadTotalMb,
  getSummaryModelSizeLabel,
  getSummaryModelSizeMb,
  resolveOnboardingSummaryModelStatus,
} from '@/lib/onboarding-summary-model';

// Ported from tests/lib/onboarding-summary-model.test.mjs (a hand-rolled
// node:vm + node:assert script with no test runner, so it never ran via npm).
describe('resolveOnboardingSummaryModelStatus', () => {
  it('does not mark an undownloaded selected model as ready regardless of recommendation', () => {
    expect(
      resolveOnboardingSummaryModelStatus({
        selectedModel: 'qwen3.5:4b',
        recommendedModel: 'qwen3.5:4b',
        selectedModelReady: false,
      }),
    ).toEqual({
      selectedSummaryModel: 'qwen3.5:4b',
      summaryModelDownloaded: false,
    });
  });

  it('lets an explicitly selected, ready model win over a different recommendation', () => {
    expect(
      resolveOnboardingSummaryModelStatus({
        selectedModel: 'gemma3:1b',
        recommendedModel: 'qwen3.5:4b',
        selectedModelReady: true,
      }),
    ).toEqual({
      selectedSummaryModel: 'gemma3:1b',
      summaryModelDownloaded: true,
    });
  });

  it('falls back to the recommended model when nothing is selected', () => {
    expect(
      resolveOnboardingSummaryModelStatus({
        selectedModel: '',
        recommendedModel: 'qwen3.5:2b',
        selectedModelReady: true,
      }),
    ).toEqual({
      selectedSummaryModel: 'qwen3.5:2b',
      summaryModelDownloaded: true,
    });
  });

  it('reports not-downloaded when selectedModel is empty and ready flag is meaningless', () => {
    expect(
      resolveOnboardingSummaryModelStatus({
        selectedModel: '',
        recommendedModel: '',
        selectedModelReady: true,
      }),
    ).toEqual({
      selectedSummaryModel: '',
      summaryModelDownloaded: false,
    });
  });
});

describe('getSummaryModelSizeMb', () => {
  it('returns known sizes', () => {
    expect(getSummaryModelSizeMb('qwen3.5:2b')).toBe(1221);
    expect(getSummaryModelSizeMb('qwen3.5:4b')).toBe(2614);
    expect(getSummaryModelSizeMb('ruadapt-qwen3:4b')).toBe(2490);
    expect(getSummaryModelSizeMb('gemma3:1b')).toBe(1019);
  });

  it('returns 0 for unknown models', () => {
    expect(getSummaryModelSizeMb('unknown:model')).toBe(0);
  });
});

describe('getSummaryModelSizeLabel', () => {
  it('formats GiB for >= 1024 MiB', () => {
    expect(getSummaryModelSizeLabel('qwen3.5:2b')).toBe('~1.2 GiB');
    expect(getSummaryModelSizeLabel('qwen3.5:4b')).toBe('~2.6 GiB');
  });

  it('returns empty string for unknown models', () => {
    expect(getSummaryModelSizeLabel('unknown:model')).toBe('');
  });
});

describe('getDownloadTotalMb', () => {
  it('uses the explicit total when provided', () => {
    expect(getDownloadTotalMb(512, 'qwen3.5:4b')).toBe(512);
  });

  it('falls back to the model size when total is 0 or undefined', () => {
    expect(getDownloadTotalMb(0, 'qwen3.5:4b')).toBe(2614);
    expect(getDownloadTotalMb(undefined, 'qwen3.5:2b')).toBe(1221);
  });
});
