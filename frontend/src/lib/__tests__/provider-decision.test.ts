import { describe, it, expect } from 'vitest';
import {
  CLOUD_PROVIDERS,
  isActionable,
  toBackendPayload,
  decisionFromStatusMarker,
  type SummaryProviderDecision,
} from '@/types/provider-decision';

describe('CLOUD_PROVIDERS', () => {
  it('lists Caila first (Russian-market default)', () => {
    expect(CLOUD_PROVIDERS[0]).toBe('caila');
  });

  it('does not include ollama (local server, belongs in Settings)', () => {
    expect(CLOUD_PROVIDERS).not.toContain('ollama' as never);
  });
});

describe('isActionable', () => {
  const local: SummaryProviderDecision = { kind: 'local', model: 'qwen3.5:4b' };
  const cloud: SummaryProviderDecision = {
    kind: 'cloud',
    provider: 'caila',
    apiKey: 'k',
    model: 'm',
  };
  const deferred: SummaryProviderDecision = {
    kind: 'deferred',
    reason: 'user_skipped',
  };

  it('returns true for local', () => {
    expect(isActionable(local)).toBe(true);
  });
  it('returns true for cloud', () => {
    expect(isActionable(cloud)).toBe(true);
  });
  it('returns false for deferred', () => {
    expect(isActionable(deferred)).toBe(false);
  });
  it('returns false for null', () => {
    expect(isActionable(null)).toBe(false);
  });
  // Type-narrowing smoke test — compiles only if isActionable is a guard.
  it('narrows to a non-deferred decision', () => {
    const d: SummaryProviderDecision | null = cloud;
    if (isActionable(d)) {
      // d.model is available on both local and cloud branches.
      expect(typeof d.model).toBe('string');
    }
  });
});

describe('toBackendPayload', () => {
  it('maps local preserving the model', () => {
    expect(toBackendPayload({ kind: 'local', model: 'qwen3.5:4b' })).toEqual({
      kind: 'local',
      model: 'qwen3.5:4b',
    });
  });

  it('maps cloud with snake_case api_key and the raw key', () => {
    expect(
      toBackendPayload({
        kind: 'cloud',
        provider: 'caila',
        apiKey: 'secret',
        model: 'm',
      })
    ).toEqual({
      kind: 'cloud',
      provider: 'caila',
      api_key: 'secret',
      model: 'm',
    });
  });

  it('maps deferred → skip (Rust enum variant name)', () => {
    expect(
      toBackendPayload({ kind: 'deferred', reason: 'user_skipped' })
    ).toEqual({ kind: 'skip' });
  });
});

describe('decisionFromStatusMarker', () => {
  it('parses "local" using the fallback model', () => {
    expect(decisionFromStatusMarker('local', 'qwen3.5:2b')).toEqual({
      kind: 'local',
      model: 'qwen3.5:2b',
    });
  });

  it('parses "deferred"', () => {
    expect(decisionFromStatusMarker('deferred', 'ignored')).toEqual({
      kind: 'deferred',
      reason: 'user_skipped',
    });
  });

  it('parses "cloud:caila"', () => {
    expect(decisionFromStatusMarker('cloud:caila', 'ignored')).toEqual({
      kind: 'cloud',
      provider: 'caila',
      apiKey: '',
      model: '',
    });
  });

  it('returns null for undefined / null / unknown marker', () => {
    expect(decisionFromStatusMarker(null, 'm')).toBeNull();
    expect(decisionFromStatusMarker(undefined, 'm')).toBeNull();
    expect(decisionFromStatusMarker('garbage', 'm')).toBeNull();
  });
});
