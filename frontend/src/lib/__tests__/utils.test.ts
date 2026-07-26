import { describe, it, expect } from 'vitest';
import { cn, isOllamaNotInstalledError } from '@/lib/utils';

describe('cn', () => {
  it('merges class names', () => {
    expect(cn('a', 'b')).toBe('a b');
  });

  it('dedupes conflicting tailwind classes keeping the last', () => {
    // twMerge resolves the `px-*` conflict in favour of the latter.
    expect(cn('px-2', 'px-4')).toBe('px-4');
  });

  it('skips falsy values', () => {
    expect(cn('a', false, null, undefined, '', 'b')).toBe('a b');
  });

  it('handles conditional arrays', () => {
    expect(cn(['a', { b: true, c: false }])).toBe('a b');
  });

  it('returns empty string for no input', () => {
    expect(cn()).toBe('');
  });
});

describe('isOllamaNotInstalledError', () => {
  const positive = [
    'Cannot connect to ollama',
    'Connection refused',
    'ollama cli not found',
    'not in path',
    'Please check if the ollama server is running',
    'ECONNREFUSED 127.0.0.1:11434',
    'not found or not in path',
  ];

  it.each(positive)('returns true for %j', (msg) => {
    expect(isOllamaNotInstalledError(msg)).toBe(true);
  });

  it('is case-insensitive', () => {
    expect(isOllamaNotInstalledError('CANNOT CONNECT')).toBe(true);
    expect(isOllamaNotInstalledError('EconnRefused')).toBe(true);
  });

  it('returns false for unrelated errors', () => {
    expect(isOllamaNotInstalledError('model not found')).toBe(false);
    expect(isOllamaNotInstalledError('timeout reading stream')).toBe(false);
  });

  it('returns false for empty/null-ish input', () => {
    expect(isOllamaNotInstalledError('')).toBe(false);
  });
});
