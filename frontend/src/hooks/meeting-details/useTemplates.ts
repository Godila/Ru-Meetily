import { useState, useEffect, useCallback } from 'react';
import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import Analytics from '@/lib/analytics';

/** Public template metadata, mirrors Rust `TemplateInfo`. */
export interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  /** Built-in / bundled templates are read-only; the editor offers "Save as copy". */
  is_protected?: boolean;
}

/** Format options for a section — must stay in sync with Rust `TemplateSection::validate`. */
export type SectionFormat = 'paragraph' | 'list' | 'string';

/** One section of a template, mirrors Rust `TemplateSection`. */
export interface TemplateSection {
  title: string;
  instruction: string;
  format: SectionFormat;
  /** Optional markdown hint for list items (e.g. table header). */
  item_format?: string;
}

/** Editable template shape sent to the backend on create/update.
 *  Mirrors Rust `TemplateDraft`. The id is NOT part of the draft — the
 *  backend derives it from the name on create, or takes it as a separate
 *  argument on update. */
export interface TemplateDraft {
  name: string;
  description: string;
  sections: TemplateSection[];
}

/** What the editor is working on: either a brand-new template, or an edit
 *  of an existing id. For protected templates the editor opens in "copy" mode
 *  so the built-in is never mutated. Carrying the full TemplateInfo lets the
 *  dialog prefill name/description even for built-ins (which have no custom
 *  JSON on disk). */
export type EditorTarget =
  | { mode: 'create' }
  | { mode: 'edit'; id: string; isProtected: boolean; info: TemplateInfo };

export function useTemplates() {
  const [availableTemplates, setAvailableTemplates] = useState<TemplateInfo[]>([]);
  const [selectedTemplate, setSelectedTemplate] = useState<string>('standard_meeting');
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorTarget, setEditorTarget] = useState<EditorTarget | null>(null);

  // Fetch available templates on mount
  useEffect(() => {
    void reloadTemplates();
  }, []);

  const reloadTemplates = useCallback(async (): Promise<TemplateInfo[]> => {
    try {
      const templates = await invokeTauri<TemplateInfo[]>('api_list_templates');
      setAvailableTemplates(templates);
      return templates;
    } catch (error) {
      console.error('Failed to fetch templates:', error);
      toast.error('Не удалось загрузить шаблоны', {
        description: error instanceof Error ? error.message : String(error),
      });
      return [];
    }
  }, []);

  // Handle template selection
  const handleTemplateSelection = useCallback((templateId: string, templateName: string) => {
    setSelectedTemplate(templateId);
    toast.success('Шаблон выбран', {
      description: `Для саммари используется «${templateName}»`,
    });
    Analytics.trackFeatureUsed('template_selected');
  }, []);

  /** Open the editor to create a brand-new template. */
  const openCreateEditor = useCallback(() => {
    setEditorTarget({ mode: 'create' });
    setEditorOpen(true);
  }, []);

  /** Open the editor on an existing template. Protected templates open in
   *  "copy" mode (a new id is derived on save) so built-ins stay read-only. */
  const openEditEditor = useCallback((template: TemplateInfo) => {
    setEditorTarget({
      mode: 'edit',
      id: template.id,
      isProtected: template.is_protected ?? false,
      info: template,
    });
    setEditorOpen(true);
  }, []);

  const closeEditor = useCallback(() => {
    setEditorOpen(false);
    setEditorTarget(null);
  }, []);

  /** Create a new custom template. Backend derives the id from the name. */
  const createTemplate = useCallback(async (draft: TemplateDraft): Promise<TemplateInfo | null> => {
    try {
      const created = await invokeTauri<TemplateInfo>('api_create_custom_template', { draft });
      toast.success('Шаблон создан', { description: `«${created.name}» сохранён` });
      await reloadTemplates();
      setSelectedTemplate(created.id);
      return created;
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error('Не удалось создать шаблон', { description: msg });
      return null;
    }
  }, [reloadTemplates]);

  /** Update an existing custom template by id. */
  const updateTemplate = useCallback(async (
    templateId: string,
    draft: TemplateDraft,
  ): Promise<TemplateInfo | null> => {
    try {
      const updated = await invokeTauri<TemplateInfo>('api_update_custom_template', {
        templateId,
        draft,
      });
      toast.success('Шаблон обновлён', { description: `«${updated.name}» сохранён` });
      await reloadTemplates();
      setSelectedTemplate(updated.id);
      return updated;
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error('Не удалось обновить шаблон', { description: msg });
      return null;
    }
  }, [reloadTemplates]);

  /** Delete a custom template by id. Refuses built-in/bundled ids on the backend. */
  const deleteTemplate = useCallback(async (templateId: string): Promise<boolean> => {
    try {
      await invokeTauri('api_delete_custom_template', { templateId });
      toast.success('Шаблон удалён');
      const remaining = await reloadTemplates();
      // If we deleted the currently-selected template, fall back to the default
      setSelectedTemplate((current) =>
        current === templateId
          ? remaining.find((t) => t.id === 'standard_meeting')?.id ?? remaining[0]?.id ?? 'standard_meeting'
          : current,
      );
      return true;
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error('Не удалось удалить шаблон', { description: msg });
      return false;
    }
  }, [reloadTemplates]);

  /** Fetch the full JSON of a custom template for the editor's edit mode. */
  const fetchCustomTemplateJson = useCallback(async (templateId: string): Promise<string | null> => {
    try {
      return await invokeTauri<string | null>('api_get_custom_template_json', { templateId });
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast.error('Не удалось загрузить шаблон', { description: msg });
      return null;
    }
  }, []);

  return {
    // selection
    availableTemplates,
    selectedTemplate,
    handleTemplateSelection,
    // editor lifecycle
    editorOpen,
    editorTarget,
    openCreateEditor,
    openEditEditor,
    closeEditor,
    // CRUD
    createTemplate,
    updateTemplate,
    deleteTemplate,
    fetchCustomTemplateJson,
    reloadTemplates,
  };
}
