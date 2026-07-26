import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

// `vi.hoisted` guarantees the mock fn exists *before* `vi.mock` is evaluated
// (vitest hoists `vi.mock` calls above all imports). Without this, the factory
// would close over an uninitialised binding.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// `sonner` toasts render into a portal and pull in React DOM work that is not
// relevant to this hook's logic. Stub it as a no-op so the test stays focused.
vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  },
}));

// The hook imports Analytics (which itself imports `invoke`); stub the module
// so we never reach a real bridge from a transitive import.
vi.mock('@/lib/analytics', () => ({
  default: {
    trackFeatureUsed: vi.fn(),
  },
}));

import { useTemplates, type TemplateInfo, type TemplateDraft } from '@/hooks/meeting-details/useTemplates';

const STANDARD: TemplateInfo = { id: 'standard_meeting', name: 'Standard', description: 'd' };

// Reset between tests so `mockResolvedValueOnce` sequences from one test never
// bleed into another. The default resolves any call to the STANDARD list, which
// makes the mount-time `api_list_templates` effect harmless by itself; tests
// that need a specific call sequence prepend `mockResolvedValueOnce` calls,
// which take priority over this default.
beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue([STANDARD]);
});

// Flush the mount-time `reloadTemplates()` effect. We wait one macrotask tick
// rather than using `vi.waitFor` because the hook's effect schedules a state
// update inside a microtask; a fixed delay is deterministic and avoids the
// retry/poll interaction that made `waitFor` flaky here.
async function flushMount() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 50));
  });
}

// Render the hook and flush its initial load effect. Returns the hook's
// `result.current` ref so callers can read state directly.
async function renderAndLoad() {
  const { result } = renderHook(() => useTemplates());
  await flushMount();
  return result;
}

describe('useTemplates — initial load', () => {
  it('fetches templates on mount and exposes them', async () => {
    const result = await renderAndLoad();

    expect(invokeMock).toHaveBeenCalledWith('api_list_templates');
    expect(result.current.availableTemplates[0]).toEqual(STANDARD);
    // The default selection is the standard_meeting template.
    expect(result.current.selectedTemplate).toBe('standard_meeting');
  });
});

describe('useTemplates — selection', () => {
  it('handleTemplateSelection updates the selected template id', async () => {
    const result = await renderAndLoad();

    act(() => {
      result.current.handleTemplateSelection('custom_x', 'My custom');
    });

    expect(result.current.selectedTemplate).toBe('custom_x');
  });
});

describe('useTemplates — editor lifecycle', () => {
  it('openCreateEditor opens the editor in create mode', async () => {
    const result = await renderAndLoad();

    expect(result.current.editorOpen).toBe(false);
    expect(result.current.editorTarget).toBeNull();

    act(() => result.current.openCreateEditor());

    expect(result.current.editorOpen).toBe(true);
    expect(result.current.editorTarget).toEqual({ mode: 'create' });
  });

  it('openEditEditor opens the editor on an existing template, preserving isProtected', async () => {
    const result = await renderAndLoad();

    const protectedTpl: TemplateInfo = { id: 'standard_meeting', name: 'Standard', description: 'd', is_protected: true };
    act(() => result.current.openEditEditor(protectedTpl));

    expect(result.current.editorOpen).toBe(true);
    expect(result.current.editorTarget).toEqual({
      mode: 'edit',
      id: 'standard_meeting',
      isProtected: true,
      info: protectedTpl,
    });
  });

  it('closeEditor resets both open state and target', async () => {
    const result = await renderAndLoad();

    act(() => result.current.openCreateEditor());
    act(() => result.current.closeEditor());

    expect(result.current.editorOpen).toBe(false);
    expect(result.current.editorTarget).toBeNull();
  });
});

describe('useTemplates — CRUD', () => {
  const draft: TemplateDraft = {
    name: 'Standup',
    description: 'Daily standup',
    sections: [{ title: 'Blockers', instruction: 'List blockers', format: 'list' }],
  };

  it('createTemplate calls api_create_custom_template with the draft and selects the new id', async () => {
    const created: TemplateInfo = { id: 'standup', name: 'Standup', description: 'Daily standup' };
    // Sequence of calls: mount-reload, create, post-create-reload.
    invokeMock.mockResolvedValueOnce([STANDARD]);
    invokeMock.mockResolvedValueOnce(created);
    invokeMock.mockResolvedValueOnce([STANDARD, created]);

    const result = await renderAndLoad();

    let created_: TemplateInfo | null = null;
    await act(async () => {
      created_ = await result.current.createTemplate(draft);
    });

    expect(created_).toEqual(created);
    expect(invokeMock).toHaveBeenCalledWith('api_create_custom_template', { draft });
    expect(result.current.selectedTemplate).toBe('standup');
  });

  it('createTemplate returns null and does not throw on backend error', async () => {
    invokeMock.mockResolvedValueOnce([STANDARD]); // mount reload
    invokeMock.mockRejectedValueOnce(new Error('bad name')); // create fails

    const result = await renderAndLoad();

    let outcome: TemplateInfo | null = 'sentinel' as unknown as TemplateInfo | null;
    await act(async () => {
      outcome = await result.current.createTemplate(draft);
    });

    expect(outcome).toBeNull();
  });

  it('updateTemplate calls api_update_custom_template with id + draft', async () => {
    const updated: TemplateInfo = { id: 'standup', name: 'Standup v2', description: 'x' };
    invokeMock.mockResolvedValueOnce([STANDARD]); // mount reload
    invokeMock.mockResolvedValueOnce(updated); // update
    invokeMock.mockResolvedValueOnce([STANDARD, updated]); // post-update reload

    const result = await renderAndLoad();

    let res: TemplateInfo | null = null;
    await act(async () => {
      res = await result.current.updateTemplate('standup', draft);
    });

    expect(res).toEqual(updated);
    expect(invokeMock).toHaveBeenCalledWith('api_update_custom_template', {
      templateId: 'standup',
      draft,
    });
    expect(result.current.selectedTemplate).toBe('standup');
  });

  it('deleteTemplate calls api_delete_custom_template and falls back to standard_meeting when deleting the selection', async () => {
    // Mount with two templates so we can select the non-default one and delete it.
    const standup: TemplateInfo = { id: 'standup', name: 'Standup', description: '' };
    invokeMock.mockResolvedValueOnce([STANDARD, standup]); // mount
    invokeMock.mockResolvedValueOnce(undefined); // delete
    invokeMock.mockResolvedValueOnce([STANDARD]); // post-delete reload

    const result = await renderAndLoad();
    act(() => result.current.handleTemplateSelection('standup', 'Standup'));

    let ok = false;
    await act(async () => {
      ok = await result.current.deleteTemplate('standup');
    });

    expect(ok).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('api_delete_custom_template', { templateId: 'standup' });
    // We deleted the selection -> it falls back to the default.
    expect(result.current.selectedTemplate).toBe('standard_meeting');
  });

  it('deleteTemplate returns false on backend error', async () => {
    invokeMock.mockResolvedValueOnce([STANDARD]); // mount
    invokeMock.mockRejectedValueOnce(new Error('protected')); // delete fails

    const result = await renderAndLoad();

    let ok = true;
    await act(async () => {
      ok = await result.current.deleteTemplate('standard_meeting');
    });

    expect(ok).toBe(false);
  });

  it('fetchCustomTemplateJson returns the raw JSON string from the backend', async () => {
    invokeMock.mockResolvedValueOnce([STANDARD]); // mount
    invokeMock.mockResolvedValueOnce('{"name":"Standard"}'); // get json

    const result = await renderAndLoad();

    let json: string | null = null;
    await act(async () => {
      json = await result.current.fetchCustomTemplateJson('standard_meeting');
    });

    expect(json).toBe('{"name":"Standard"}');
    expect(invokeMock).toHaveBeenCalledWith('api_get_custom_template_json', {
      templateId: 'standard_meeting',
    });
  });
});
