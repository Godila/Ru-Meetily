"use client";

import { useEffect, useState, useCallback, useRef } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion';
import { Plus, Trash2, GripVertical, Loader2, Save, Copy, AlertCircle } from 'lucide-react';
import { toast } from 'sonner';
import type {
  EditorTarget,
  SectionFormat,
  TemplateDraft,
  TemplateInfo,
  TemplateSection,
} from '@/hooks/meeting-details/useTemplates';

interface TemplateEditorDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  target: EditorTarget | null;
  /** Read the on-disk JSON of an existing custom template (null if none). */
  loadExisting: (id: string) => Promise<string | null>;
  onCreate: (draft: TemplateDraft) => Promise<TemplateInfo | null>;
  onUpdate: (id: string, draft: TemplateDraft) => Promise<TemplateInfo | null>;
  onDelete?: (id: string) => Promise<boolean>;
}

const FORMATS: { value: SectionFormat; label: string }[] = [
  { value: 'paragraph', label: 'Абзац' },
  { value: 'list', label: 'Список' },
  { value: 'string', label: 'Строка' },
];

const EMPTY_SECTION: TemplateSection = {
  title: '',
  instruction: '',
  format: 'paragraph',
  item_format: '',
};

const EMPTY_DRAFT: TemplateDraft = {
  name: '',
  description: '',
  sections: [{ ...EMPTY_SECTION }],
};

/** Whether this edit session must produce a NEW id (built-in → copy, or create). */
function isCopyMode(target: EditorTarget | null): boolean {
  if (!target) return false;
  return target.mode === 'create' || (target.mode === 'edit' && target.isProtected);
}

export function TemplateEditorDialog({
  open,
  onOpenChange,
  target,
  loadExisting,
  onCreate,
  onUpdate,
  onDelete,
}: TemplateEditorDialogProps) {
  const [draft, setDraft] = useState<TemplateDraft>(EMPTY_DRAFT);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  // Track closed→open transitions so re-opening on a different target resets state.
  // (Same pattern as ImportAudioDialog — avoids stale form across opens.)
  const prevOpenRef = useRef(false);

  // Reset + load existing content when the dialog opens
  useEffect(() => {
    const wasOpen = prevOpenRef.current;
    prevOpenRef.current = open;
    if (!open || wasOpen) return;
    if (!target) return;

    let cancelled = false;
    (async () => {
      setLoading(true);
      setDraft(EMPTY_DRAFT);

      if (target.mode === 'create') {
        setLoading(false);
        return;
      }

      // Edit mode: prefer the custom on-disk JSON (exact content). For a
      // protected template with no custom file, prefill name/description from
      // the TemplateInfo we already have (the dropdown had it) so the user
      // starts from the built-in's metadata rather than a blank form. Sections
      // cannot be reconstructed from TemplateInfo and are left for the user.
      try {
        const customJson = await loadExisting(target.id);
        if (cancelled) return;

        if (customJson) {
          const parsed = JSON.parse(customJson) as TemplateDraft;
          setDraft({
            name: parsed.name ?? '',
            description: parsed.description ?? '',
            sections: Array.isArray(parsed.sections) && parsed.sections.length > 0
              ? parsed.sections
              : [{ ...EMPTY_SECTION }],
          });
        } else if (target.mode === 'edit') {
          // Built-in/bundled with no custom override: prefill metadata, leave
          // sections empty for the user to author (we don't have instructions).
          setDraft({
            name: target.isProtected ? `${target.info.name} (копия)` : target.info.name,
            description: target.info.description,
            sections: [{ ...EMPTY_SECTION }],
          });
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        toast.error('Не удалось загрузить шаблон', { description: msg });
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [open, target, loadExisting]);

  const copyMode = isCopyMode(target);

  const updateField = useCallback(<K extends keyof TemplateDraft>(key: K, value: TemplateDraft[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }));
  }, []);

  const updateSection = useCallback((index: number, patch: Partial<TemplateSection>) => {
    setDraft((prev) => ({
      ...prev,
      sections: prev.sections.map((s, i) => (i === index ? { ...s, ...patch } : s)),
    }));
  }, []);

  const addSection = useCallback(() => {
    setDraft((prev) => ({ ...prev, sections: [...prev.sections, { ...EMPTY_SECTION }] }));
  }, []);

  const removeSection = useCallback((index: number) => {
    setDraft((prev) => ({
      ...prev,
      sections: prev.sections.filter((_, i) => i !== index),
    }));
  }, []);

  const handleSave = useCallback(async () => {
    // Basic client-side guard; the backend re-validates authoritatively.
    if (!draft.name.trim()) {
      toast.error('Укажите название шаблона');
      return;
    }
    if (draft.sections.length === 0 || draft.sections.some((s) => !s.title.trim() || !s.instruction.trim())) {
      toast.error('Каждая секция должна иметь заголовок и инструкцию');
      return;
    }

    setSaving(true);
    try {
      const cleaned: TemplateDraft = {
        name: draft.name.trim(),
        description: draft.description.trim(),
        sections: draft.sections.map((s) => ({
          title: s.title.trim(),
          instruction: s.instruction.trim(),
          format: s.format,
          // Only send item_format when relevant and non-empty
          ...(s.format === 'list' && s.item_format?.trim() ? { item_format: s.item_format.trim() } : {}),
        })),
      };

      // copyMode is true for create-mode AND for protected edit-mode; both
      // produce a new template. Only a non-protected edit updates in place.
      // The non-null/non-create assertion is safe: copyMode is the only
      // create-mode branch, so the else branch always has an edit-mode target.
      const result = copyMode
        ? await onCreate(cleaned)
        : await onUpdate((target as { id: string }).id, cleaned);

      if (result) {
        onOpenChange(false);
      }
    } finally {
      setSaving(false);
    }
  }, [draft, copyMode, target, onCreate, onUpdate, onOpenChange]);

  const handleDelete = useCallback(async () => {
    if (!target || target.mode !== 'edit' || target.isProtected || !onDelete) return;
    if (!confirm(`Удалить шаблон «${draft.name}»? Действие нельзя отменить.`)) return;
    const ok = await onDelete(target.id);
    if (ok) onOpenChange(false);
  }, [target, draft.name, onDelete, onOpenChange]);

  const canSave = !loading && !saving && draft.name.trim().length > 0 && draft.sections.length > 0;

  const titleText = copyMode
    ? 'Создать копию шаблона'
    : target?.mode === 'edit'
      ? 'Редактировать шаблон'
      : 'Новый шаблон';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[640px] max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {copyMode ? <Copy className="h-5 w-5 text-blue-600" /> : <Save className="h-5 w-5 text-blue-600" />}
            {titleText}
          </DialogTitle>
          <DialogDescription>
            {copyMode
              ? 'Внесённые изменения сохранятся как новый пользовательский шаблон.'
              : 'Настройте структуру саммари: секции и инструкции для модели.'}
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-6 w-6 animate-spin text-blue-600" />
          </div>
        ) : (
          <div className="space-y-4 py-2">
            {target?.mode === 'edit' && target.isProtected && (
              <div className="flex items-start gap-2 rounded-md bg-amber-50 border border-amber-200 p-3 text-sm text-amber-900">
                <AlertCircle className="h-4 w-4 mt-0.5 flex-shrink-0" />
                <span>
                  Это встроенный шаблон. Его нельзя изменить напрямую — будет создана
                  пользовательская копия.
                </span>
              </div>
            )}

            <div className="space-y-2">
              <Label htmlFor="tpl-name">Название</Label>
              <Input
                id="tpl-name"
                value={draft.name}
                onChange={(e) => updateField('name', e.target.value)}
                placeholder="Напр. Еженедельная синхронизация"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="tpl-desc">Описание</Label>
              <Input
                id="tpl-desc"
                value={draft.description}
                onChange={(e) => updateField('description', e.target.value)}
                placeholder="Короткое описание назначения шаблона"
              />
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Секции</Label>
                <Button variant="outline" size="sm" onClick={addSection} type="button">
                  <Plus className="h-4 w-4 mr-1" />
                  Добавить
                </Button>
              </div>

              {draft.sections.length === 0 ? (
                <p className="text-sm text-muted-foreground">Добавьте хотя бы одну секцию.</p>
              ) : (
                <Accordion type="multiple" className="w-full">
                  {draft.sections.map((section, index) => (
                    <AccordionItem key={index} value={`section-${index}`} className="border rounded-md px-3 mb-2">
                      <div className="flex items-center gap-2">
                        <GripVertical className="h-4 w-4 text-muted-foreground flex-shrink-0" />
                        <AccordionTrigger className="flex-1 hover:no-underline text-left">
                          <span className="truncate">
                            {section.title.trim() || `Секция ${index + 1}`}
                          </span>
                        </AccordionTrigger>
                        {draft.sections.length > 1 && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={(e) => {
                              e.stopPropagation();
                              removeSection(index);
                            }}
                            title="Удалить секцию"
                            type="button"
                          >
                            <Trash2 className="h-4 w-4 text-red-500" />
                          </Button>
                        )}
                      </div>
                      <AccordionContent className="space-y-3 pt-2">
                        <div className="space-y-1">
                          <Label htmlFor={`sec-title-${index}`} className="text-xs">Заголовок</Label>
                          <Input
                            id={`sec-title-${index}`}
                            value={section.title}
                            onChange={(e) => updateSection(index, { title: e.target.value })}
                            placeholder="Напр. Ключевые решения"
                          />
                        </div>
                        <div className="space-y-1">
                          <Label htmlFor={`sec-instr-${index}`} className="text-xs">Инструкция для модели</Label>
                          <Textarea
                            id={`sec-instr-${index}`}
                            value={section.instruction}
                            onChange={(e) => updateSection(index, { instruction: e.target.value })}
                            placeholder="Что модель должна извлечь в эту секцию"
                            rows={3}
                          />
                        </div>
                        <div className="space-y-1">
                          <Label className="text-xs">Формат</Label>
                          <Select
                            value={section.format}
                            onValueChange={(v) => updateSection(index, { format: v as SectionFormat })}
                          >
                            <SelectTrigger className="w-full">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              {FORMATS.map((f) => (
                                <SelectItem key={f.value} value={f.value}>
                                  {f.label}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                        </div>
                        {section.format === 'list' && (
                          <div className="space-y-1">
                            <Label htmlFor={`sec-itemfmt-${index}`} className="text-xs">
                              Формат элементов (необязательно)
                            </Label>
                            <Textarea
                              id={`sec-itemfmt-${index}`}
                              value={section.item_format ?? ''}
                              onChange={(e) => updateSection(index, { item_format: e.target.value })}
                              placeholder="| Колонка 1 | Колонка 2 |&#10;| --- | --- |"
                              rows={2}
                              className="font-mono text-xs"
                            />
                          </div>
                        )}
                      </AccordionContent>
                    </AccordionItem>
                  ))}
                </Accordion>
              )}
            </div>
          </div>
        )}

        <DialogFooter className="gap-2 sm:gap-2">
          {!copyMode && target?.mode === 'edit' && !target.isProtected && onDelete && (
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={saving}
              className="mr-auto"
              type="button"
            >
              <Trash2 className="h-4 w-4 mr-1" />
              Удалить
            </Button>
          )}
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving} type="button">
            Отмена
          </Button>
          <Button onClick={handleSave} disabled={!canSave} type="button">
            {saving ? (
              <>
                <Loader2 className="h-4 w-4 mr-1 animate-spin" />
                Сохранение…
              </>
            ) : copyMode ? (
              <>
                <Copy className="h-4 w-4 mr-1" />
                Сохранить копию
              </>
            ) : (
              <>
                <Save className="h-4 w-4 mr-1" />
                Сохранить
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
