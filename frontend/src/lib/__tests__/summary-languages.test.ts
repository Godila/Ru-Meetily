import { describe, it, expect } from 'vitest';
import {
  normaliseLanguageCode,
  labelForCode,
  LANGUAGE_OPTIONS,
  AUTO_VALUE,
} from '@/lib/summary-languages';

describe('normaliseLanguageCode', () => {
  it('lowercases a plain supported code', () => {
    expect(normaliseLanguageCode('EN')).toBe('en');
    expect(normaliseLanguageCode('RU')).toBe('ru');
  });

  it('strips a BCP-47 region subtag when the base is supported', () => {
    // pt-BR -> pt (we only ship 'pt')
    expect(normaliseLanguageCode('pt-BR')).toBe('pt');
    expect(normaliseLanguageCode('en_GB')).toBe('en'); // underscore -> dash, then base
    expect(normaliseLanguageCode('zh-CN')).toBe('zh');
  });

  it('keeps the full code when it is itself supported', () => {
    // zh-tw is a first-class option, so the full tag must survive.
    expect(normaliseLanguageCode('zh-TW')).toBe('zh-tw');
  });

  it('returns null for unsupported languages', () => {
    expect(normaliseLanguageCode('xx')).toBeNull();
    expect(normaliseLanguageCode('xx-YY')).toBeNull();
  });

  it('returns null for empty / null / undefined', () => {
    expect(normaliseLanguageCode('')).toBeNull();
    expect(normaliseLanguageCode(null)).toBeNull();
    expect(normaliseLanguageCode(undefined)).toBeNull();
  });

  it('returns null when the base is unsupported even with a region tag', () => {
    expect(normaliseLanguageCode('xx-BR')).toBeNull();
  });
});

describe('labelForCode', () => {
  it('returns the human label for a known code', () => {
    expect(labelForCode('en')).toBe('English');
    expect(labelForCode('ru')).toBe('Russian');
    expect(labelForCode('zh-tw')).toBe('Traditional Chinese');
  });

  it('falls back to the raw code when unknown', () => {
    expect(labelForCode('xx')).toBe('xx');
  });

  it('is consistent with LANGUAGE_OPTIONS', () => {
    for (const opt of LANGUAGE_OPTIONS) {
      expect(labelForCode(opt.code)).toBe(opt.label);
    }
  });
});

describe('constants', () => {
  it('AUTO_VALUE is a stable sentinel', () => {
    expect(AUTO_VALUE).toBe('__auto__');
  });

  it('LANGUAGE_OPTIONS codes are unique', () => {
    const codes = LANGUAGE_OPTIONS.map((o) => o.code);
    expect(new Set(codes).size).toBe(codes.length);
  });
});
