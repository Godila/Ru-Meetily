import { describe, it, expect, afterEach, vi } from 'vitest';
import { blocksToMarkdownSafely } from '@/lib/blocknote-markdown';

// Ported from tests/lib/blocknote-markdown.test.ts (originally bun:test, never
// run because bun wasn't in deps). The logic under test is unchanged.
describe('blocksToMarkdownSafely', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('returns markdown when conversion succeeds', async () => {
    const editor = {
      blocksToMarkdownLossy: vi.fn(async () => '# Summary'),
    };

    const result = await blocksToMarkdownSafely(editor, [] as never, {
      source: 'test-success',
    });

    expect(result).toEqual({ markdown: '# Summary', ok: true });
    expect(editor.blocksToMarkdownLossy).toHaveBeenCalledTimes(1);
  });

  it('returns fallback markdown when conversion throws', async () => {
    const error = new Error('conversion failed');
    const editor = {
      blocksToMarkdownLossy: vi.fn(async () => {
        throw error;
      }),
    };
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    const result = await blocksToMarkdownSafely(editor, [{ id: 'block-1' }] as never, {
      source: 'test-fallback',
      fallbackMarkdown: 'existing markdown',
    });

    expect(result).toEqual({ markdown: 'existing markdown', ok: false });
    expect(consoleError).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledWith(
      'Failed to convert BlockNote blocks to markdown',
      {
        source: 'test-fallback',
        blocksCount: 1,
        error,
      },
    );
  });

  it('omits markdown when conversion throws without fallback', async () => {
    const editor = {
      blocksToMarkdownLossy: vi.fn(async () => {
        throw new Error('conversion failed');
      }),
    };
    vi.spyOn(console, 'error').mockImplementation(() => {});

    const result = await blocksToMarkdownSafely(editor, [] as never, {
      source: 'test-empty-fallback',
    });

    expect(result).toEqual({ markdown: undefined, ok: false });
  });
});
