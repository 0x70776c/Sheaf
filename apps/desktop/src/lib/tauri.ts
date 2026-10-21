import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  CommandErrorPayload,
  InfoReport,
  ProgressEventPayload,
  ThumbnailItem,
} from './types';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function pickPdfFiles(multiple = true): Promise<string[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  const selected = await open({
    multiple,
    filters: [{ name: 'PDF Documents', extensions: ['pdf'] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function openPdf(paths: string[]): Promise<InfoReport> {
  return await invoke<InfoReport>('open_pdf', { paths });
}

export async function fetchThumbnails(
  path: string,
  pages: number[],
  widthPx = 160,
): Promise<ThumbnailItem[]> {
  return await invoke<ThumbnailItem[]>('thumbnails', {
    path,
    pages,
    widthPx,
  });
}

export async function cancelActiveOperation(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke('cancel_op');
}

export async function onProgressEvent(
  handler: (payload: ProgressEventPayload) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return await listen<ProgressEventPayload>('sheaf://progress', (event) => {
    handler(event.payload);
  });
}

export function extractErrorMessage(err: unknown): CommandErrorPayload {
  if (typeof err === 'object' && err !== null && 'message' in err) {
    const obj = err as Record<string, unknown>;
    return {
      code: typeof obj.code === 'string' ? obj.code : 'internal',
      message: String(obj.message),
      hint: typeof obj.hint === 'string' ? obj.hint : null,
    };
  }
  return {
    code: 'internal',
    message: typeof err === 'string' ? err : 'Unexpected error occurred',
    hint: null,
  };
}
