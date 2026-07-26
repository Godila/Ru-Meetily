import { describe, it, expect, beforeEach } from 'vitest';
import {
  readPinnedSummaryLanguageDefault,
  writePinnedSummaryLanguageDefault,
  SUMMARY_LANGUAGE_DEFAULT_KEY,
} from '@/lib/summary-language-preferences';

beforeEach(() => {
  window.localStorage.clear();
});

describe('readPinnedSummaryLanguageDefault', () => {
  it('returns null when nothing is stored', () => {
    expect(readPinnedSummaryLanguageDefault()).toBeNull();
  });

  it('reads back a normalised value', () => {
    window.localStorage.setItem(SUMMARY_LANGUAGE_DEFAULT_KEY, 'EN');
    expect(readPinnedSummaryLanguageDefault()).toBe('en');
  });

  it('normalises a stored region tag to the supported base', () => {
    window.localStorage.setItem(SUMMARY_LANGUAGE_DEFAULT_KEY, 'pt-BR');
    expect(readPinnedSummaryLanguageDefault()).toBe('pt');
  });

  it('returns null for an unsupported stored value', () => {
    window.localStorage.setItem(SUMMARY_LANGUAGE_DEFAULT_KEY, 'klingon');
    expect(readPinnedSummaryLanguageDefault()).toBeNull();
  });
});

describe('writePinnedSummaryLanguageDefault', () => {
  it('persists the value verbatim', () => {
    writePinnedSummaryLanguageDefault('ru');
    expect(window.localStorage.getItem(SUMMARY_LANGUAGE_DEFAULT_KEY)).toBe('ru');
  });

  it('removes the key when given null', () => {
    writePinnedSummaryLanguageDefault('ru');
    writePinnedSummaryLanguageDefault(null);
    expect(window.localStorage.getItem(SUMMARY_LANGUAGE_DEFAULT_KEY)).toBeNull();
  });

  it('is idempotent for a null write on an empty store', () => {
    writePinnedSummaryLanguageDefault(null);
    expect(window.localStorage.getItem(SUMMARY_LANGUAGE_DEFAULT_KEY)).toBeNull();
  });
});
