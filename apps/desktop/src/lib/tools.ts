import type { Language } from './i18n';
import { translations } from './i18n';
import type { ToolId } from './types';

export interface ToolMetadata {
  id: ToolId;
  label: string;
  group: 'primary' | 'secondary';
  shortcut: string;
  actionLabel: string;
  emptyPrompt: string;
  description: string;
}

export function getLocalizedTools(lang: Language = 'en'): ToolMetadata[] {
  const t = translations[lang];
  return [
    {
      id: 'merge',
      label: t.merge,
      group: 'primary',
      shortcut: '⌘1',
      actionLabel: lang === 'de' ? 'PDFs zusammenführen' : 'Merge PDFs',
      emptyPrompt: lang === 'de' ? 'PDF-Dateien hier ablegen, um sie in Reihenfolge zu verbinden' : 'Drop PDF files here to combine them in order',
      description: t.mergeDesc,
    },
    {
      id: 'split',
      label: t.split,
      group: 'primary',
      shortcut: '⌘2',
      actionLabel: lang === 'de' ? 'PDF teilen' : 'Split PDF',
      emptyPrompt: lang === 'de' ? 'PDF-Datei hier ablegen, um Seiten oder Kapitel zu trennen' : 'Drop a PDF file here to split pages or chapters',
      description: t.splitDesc,
    },
    {
      id: 'pages',
      label: t.pages,
      group: 'primary',
      shortcut: '⌘3',
      actionLabel: lang === 'de' ? 'Seitenänderungen anwenden' : 'Apply Page Changes',
      emptyPrompt: lang === 'de' ? 'PDF-Datei hier ablegen, um Seiten zu drehen, löschen oder zu ordnen' : 'Drop a PDF file here to rotate, delete, reorder, or extract pages',
      description: t.pagesDesc,
    },
    {
      id: 'compress',
      label: t.compress,
      group: 'primary',
      shortcut: '⌘4',
      actionLabel: lang === 'de' ? 'PDF komprimieren' : 'Compress PDF',
      emptyPrompt: lang === 'de' ? 'PDF-Datei hier ablegen, um Dateigröße zu reduzieren' : 'Drop a PDF file here to reduce its file size',
      description: t.compressDesc,
    },
    {
      id: 'convert',
      label: t.convert,
      group: 'primary',
      shortcut: '⌘5',
      actionLabel: lang === 'de' ? 'Dokument konvertieren' : 'Convert Document',
      emptyPrompt: lang === 'de' ? 'PDF, PNG/JPG-Bilder oder Markdown-Datei hier ablegen' : 'Drop a PDF, PNG/JPG images, or Markdown file here to convert',
      description: t.convertDesc,
    },
    {
      id: 'watermark',
      label: t.watermark,
      group: 'secondary',
      shortcut: '⌘6',
      actionLabel: lang === 'de' ? 'Wasserzeichen anwenden' : 'Apply Watermark',
      emptyPrompt: lang === 'de' ? 'PDF-Datei hier ablegen, um ein Wasserzeichen hinzuzufügen' : 'Drop a PDF file here to add a watermark or page numbers',
      description: t.watermarkDesc,
    },
    {
      id: 'protect',
      label: t.protect,
      group: 'secondary',
      shortcut: '⌘7',
      actionLabel: lang === 'de' ? 'PDF schützen' : 'Protect PDF',
      emptyPrompt: lang === 'de' ? 'PDF-Datei hier ablegen, um sie zu verschlüsseln oder zu entsperren' : 'Drop a PDF file here to encrypt or unlock',
      description: t.protectDesc,
    },
    {
      id: 'info',
      label: lang === 'de' ? 'Info & Metadaten' : 'Info & Metadata',
      group: 'secondary',
      shortcut: '⌘8',
      actionLabel: lang === 'de' ? 'Metadaten speichern' : 'Save Metadata',
      emptyPrompt: lang === 'de' ? 'PDF-Datei hier ablegen, um Metadaten zu prüfen oder zu bereinigen' : 'Drop a PDF file here to inspect and edit metadata or sanitize',
      description: t.infoDesc,
    },
  ];
}

export const TOOLS: ToolMetadata[] = getLocalizedTools('en');

export function getToolById(id: ToolId, lang: Language = 'en'): ToolMetadata {
  const list = getLocalizedTools(lang);
  return list.find((t) => t.id === id) ?? list[0];
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(2)} MB`;
}

export function basename(filePath: string): string {
  const parts = filePath.split(/[/\\]/);
  return parts[parts.length - 1] || filePath;
}
