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

export const TOOLS: ToolMetadata[] = [
  {
    id: 'merge',
    label: 'Merge',
    group: 'primary',
    shortcut: '⌘1',
    actionLabel: 'Merge PDFs',
    emptyPrompt: 'Drop PDF files here to combine them in order',
    description: 'Combine multiple PDF documents into a single file while preserving bookmarks.',
  },
  {
    id: 'split',
    label: 'Split',
    group: 'primary',
    shortcut: '⌘2',
    actionLabel: 'Split PDF',
    emptyPrompt: 'Drop a PDF file here to split pages or chapters',
    description: 'Extract page ranges, split every N pages, or split by bookmark hierarchy.',
  },
  {
    id: 'pages',
    label: 'Pages',
    group: 'primary',
    shortcut: '⌘3',
    actionLabel: 'Apply Page Changes',
    emptyPrompt: 'Drop a PDF file here to rotate, delete, reorder, or extract pages',
    description: 'Inspect page thumbnails to rotate, delete, reorder, or extract selected pages.',
  },
  {
    id: 'compress',
    label: 'Compress',
    group: 'primary',
    shortcut: '⌘4',
    actionLabel: 'Compress PDF',
    emptyPrompt: 'Drop a PDF file here to reduce its file size',
    description: 'Optimize streams and resample embedded images with Low, Medium, or High presets.',
  },
  {
    id: 'convert',
    label: 'Convert',
    group: 'primary',
    shortcut: '⌘5',
    actionLabel: 'Convert Document',
    emptyPrompt: 'Drop a PDF, PNG/JPG images, or a Markdown (.md) file here to convert',
    description: 'Convert between PDF, PNG/JPG images, and Markdown (.md) documents.',
  },
  {
    id: 'watermark',
    label: 'Overlay',
    group: 'secondary',
    shortcut: '⌘6',
    actionLabel: 'Apply Overlay',
    emptyPrompt: 'Drop a PDF file here to add a watermark or page numbers',
    description: 'Stamp customizable text watermarks or page numbers with live preview.',
  },
  {
    id: 'protect',
    label: 'Protect',
    group: 'secondary',
    shortcut: '⌘7',
    actionLabel: 'Protect PDF',
    emptyPrompt: 'Drop a PDF file here to encrypt or unlock',
    description: 'Add or remove password protection on local PDF documents.',
  },
  {
    id: 'info',
    label: 'Info',
    group: 'secondary',
    shortcut: '⌘8',
    actionLabel: 'Verify Structure',
    emptyPrompt: 'Drop a PDF file here to inspect metadata and structure',
    description: 'Inspect PDF version, page dimensions, outlines, and structural validity.',
  },
];

export function getToolById(id: ToolId): ToolMetadata {
  return TOOLS.find((t) => t.id === id) ?? TOOLS[0];
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
