import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import type {
  CommandErrorPayload,
  CompressPresetUi,
  CompressReport,
  ConvertActionUi,
  ConvertPageSizeUi,
  ConvertReport,
  ImageFormatUi,
  InfoReport,
  MergeReport,
  PagesActionUi,
  PagesReport,
  ProgressEventPayload,
  ProtectActionUi,
  ProtectReport,
  SplitModeUi,
  SplitReport,
  ThumbnailItem,
  WatermarkPositionUi,
  WatermarkReport,
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

export async function pickImageFiles(multiple = true): Promise<string[]> {
  if (!isTauriRuntime()) return [];
  const selected = await open({
    multiple,
    filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg'] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickMarkdownFile(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await open({
    multiple: false,
    filters: [{ name: 'Markdown Document', extensions: ['md', 'markdown', 'txt'] }],
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function pickOutputDirectory(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const selected = await open({
    directory: true,
    multiple: false,
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function pickSavePdfPath(defaultName: string): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  return await save({
    defaultPath: defaultName,
    filters: [{ name: 'PDF Document', extensions: ['pdf'] }],
  });
}

export async function pickSaveMarkdownPath(defaultName: string): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  return await save({
    defaultPath: defaultName,
    filters: [{ name: 'Markdown Document', extensions: ['md'] }],
  });
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

export async function runMergeCommand(
  inputs: string[],
  out: string | null = null,
  force = true,
): Promise<MergeReport> {
  return await invoke<MergeReport>('run_merge', {
    inputs,
    out,
    force,
  });
}

export interface RunSplitParams {
  input: string;
  mode: SplitModeUi;
  ranges?: string | null;
  combine?: boolean;
  every?: number | null;
  bookmarksLevel?: number | null;
  outDir?: string | null;
  force?: boolean;
}

export async function runSplitCommand(params: RunSplitParams): Promise<SplitReport> {
  return await invoke<SplitReport>('run_split', {
    input: params.input,
    mode: params.mode,
    ranges: params.ranges ?? null,
    combine: params.combine ?? false,
    every: params.every ?? null,
    bookmarksLevel: params.bookmarksLevel ?? null,
    outDir: params.outDir ?? null,
    force: params.force ?? true,
  });
}

export interface RunPagesParams {
  input: string;
  action: PagesActionUi;
  pages?: string | null;
  degrees?: number | null;
  out?: string | null;
  force?: boolean;
}

export async function runPagesCommand(params: RunPagesParams): Promise<PagesReport> {
  return await invoke<PagesReport>('run_pages', {
    input: params.input,
    action: params.action,
    pages: params.pages ?? null,
    degrees: params.degrees ?? null,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export interface RunCompressParams {
  input: string;
  preset: CompressPresetUi;
  grayscale?: boolean;
  out?: string | null;
  force?: boolean;
}

export async function runCompressCommand(params: RunCompressParams): Promise<CompressReport> {
  return await invoke<CompressReport>('run_compress', {
    input: params.input,
    preset: params.preset,
    grayscale: params.grayscale ?? false,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export interface RunConvertParams {
  action: ConvertActionUi;
  inputs: string[];
  format?: ImageFormatUi | null;
  dpi?: number | null;
  quality?: number | null;
  pages?: string | null;
  pageSize?: ConvertPageSizeUi | null;
  marginPt?: number | null;
  out?: string | null;
  force?: boolean;
}

export async function runConvertCommand(params: RunConvertParams): Promise<ConvertReport> {
  return await invoke<ConvertReport>('run_convert', {
    action: params.action,
    inputs: params.inputs,
    format: params.format ?? null,
    dpi: params.dpi ?? null,
    quality: params.quality ?? null,
    pages: params.pages ?? null,
    pageSize: params.pageSize ?? null,
    marginPt: params.marginPt ?? null,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export interface RunWatermarkParams {
  input: string;
  text: string;
  position: WatermarkPositionUi;
  fontSize: number;
  opacity: number;
  rotationDeg?: number | null;
  color: string;
  pages?: string | null;
  out?: string | null;
  force?: boolean;
}

export async function runWatermarkCommand(
  params: RunWatermarkParams,
): Promise<WatermarkReport> {
  return await invoke<WatermarkReport>('run_watermark', {
    input: params.input,
    text: params.text,
    position: params.position,
    fontSize: params.fontSize,
    opacity: params.opacity,
    rotationDeg: params.rotationDeg ?? null,
    color: params.color,
    pages: params.pages ?? null,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export interface RunProtectParams {
  action: ProtectActionUi;
  input: string;
  password: string;
  ownerPassword?: string | null;
  allowPrint?: boolean;
  allowCopy?: boolean;
  out?: string | null;
  force?: boolean;
}

export async function runProtectCommand(params: RunProtectParams): Promise<ProtectReport> {
  return await invoke<ProtectReport>('run_protect', {
    action: params.action,
    input: params.input,
    password: params.password,
    ownerPassword: params.ownerPassword ?? null,
    allowPrint: params.allowPrint ?? true,
    allowCopy: params.allowCopy ?? true,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export async function revealInFinder(path: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke('reveal_in_finder', { path });
}

export async function openPathInSystem(path: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke('open_path', { path });
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
