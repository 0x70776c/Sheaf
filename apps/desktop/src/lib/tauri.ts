import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
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
  MetaGetReport,
  MetaSetReport,
  MetaStripReport,
  NumberPositionUi,
  NumberReport,
  OverlayModeUi,
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

export function startWindowDrag(event: MouseEvent | PointerEvent): void {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('button, input, select, textarea, a, [role="button"], .no-drag')) {
    return;
  }
  if (!isTauriRuntime()) return;
  try {
    const win = getCurrentWindow();
    void win.startDragging();
  } catch (err) {
    // Non-tauri or mock environment
  }
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

export async function openPdf(paths: string[], password?: string | null): Promise<InfoReport> {
  return await invoke<InfoReport>('open_pdf', { paths, password: password ?? null });
}

export async function fetchThumbnails(
  path: string,
  pages: number[],
  widthPx = 160,
  password?: string | null,
): Promise<ThumbnailItem[]> {
  return await invoke<ThumbnailItem[]>('thumbnails', {
    path,
    pages,
    widthPx,
    password: password ?? null,
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

export interface RunNumberParams {
  input: string;
  format: string;
  position: NumberPositionUi;
  start: number;
  fontSize: number;
  opacity: number;
  color: string;
  margin: number;
  pages?: string | null;
  out?: string | null;
  force?: boolean;
}

export async function runNumberCommand(
  params: RunNumberParams,
): Promise<NumberReport> {
  return await invoke<NumberReport>('run_number', {
    input: params.input,
    format: params.format,
    position: params.position,
    start: params.start,
    fontSize: params.fontSize,
    opacity: params.opacity,
    color: params.color,
    margin: params.margin,
    pages: params.pages ?? null,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export interface PreviewOverlayParams {
  input: string;
  page: number;
  totalPages: number;
  mode: OverlayModeUi;
  text: string;
  position: string;
  start: number;
  fontSize: number;
  opacity: number;
  color: string;
  margin: number;
  widthPx?: number;
}

export async function previewOverlayCommand(
  params: PreviewOverlayParams,
): Promise<ThumbnailItem> {
  return await invoke<ThumbnailItem>('preview_overlay', {
    input: params.input,
    page: params.page,
    totalPages: params.totalPages,
    mode: params.mode,
    text: params.text,
    position: params.position,
    start: params.start,
    fontSize: params.fontSize,
    opacity: params.opacity,
    color: params.color,
    margin: params.margin,
    widthPx: params.widthPx ?? 240,
  });
}

export interface RunProtectParams {
  action: ProtectActionUi;
  input: string;
  password: string;
  ownerPassword?: string | null;
  allowPrint?: boolean;
  allowCopy?: boolean;
  allowModify?: boolean;
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
    allowModify: params.allowModify ?? true,
    out: params.out ?? null,
    force: params.force ?? true,
  });
}

export async function getMetaCommand(
  input: string,
  password?: string | null,
): Promise<MetaGetReport> {
  return await invoke<MetaGetReport>('get_meta', {
    input,
    password: password ?? null,
  });
}

export interface RunMetaSetParams {
  input: string;
  title?: string | null;
  author?: string | null;
  subject?: string | null;
  keywords?: string | null;
  creator?: string | null;
  producer?: string | null;
  creationDate?: string | null;
  modDate?: string | null;
  custom?: [string, string][];
  unset?: string[];
  xmp?: string;
  out?: string | null;
  force?: boolean;
  dryRun?: boolean;
  password?: string | null;
}

export async function runMetaSetCommand(params: RunMetaSetParams): Promise<MetaSetReport> {
  return await invoke<MetaSetReport>('run_meta_set', {
    input: params.input,
    title: params.title ?? null,
    author: params.author ?? null,
    subject: params.subject ?? null,
    keywords: params.keywords ?? null,
    creator: params.creator ?? null,
    producer: params.producer ?? null,
    creationDate: params.creationDate ?? null,
    modDate: params.modDate ?? null,
    custom: params.custom ?? [],
    unset: params.unset ?? [],
    xmp: params.xmp ?? 'sync',
    out: params.out ?? null,
    force: params.force ?? true,
    dryRun: params.dryRun ?? false,
    password: params.password ?? null,
  });
}

export interface RunMetaStripParams {
  input: string;
  deep?: boolean;
  keep?: string[];
  out?: string | null;
  force?: boolean;
  dryRun?: boolean;
  password?: string | null;
}

export async function runMetaStripCommand(params: RunMetaStripParams): Promise<MetaStripReport> {
  return await invoke<MetaStripReport>('run_meta_strip', {
    input: params.input,
    deep: params.deep ?? false,
    keep: params.keep ?? [],
    out: params.out ?? null,
    force: params.force ?? true,
    dryRun: params.dryRun ?? false,
    password: params.password ?? null,
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
