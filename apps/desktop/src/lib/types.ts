export type ToolId =
  | 'merge'
  | 'split'
  | 'pages'
  | 'compress'
  | 'convert'
  | 'watermark'
  | 'protect'
  | 'info';

export type ThemePreference = 'system' | 'light' | 'dark';

export type SplitModeUi = 'ranges' | 'every' | 'bookmarks';
export type PagesActionUi = 'rotate' | 'delete' | 'reorder' | 'extract';
export type CompressPresetUi = 'low' | 'medium' | 'high';

export interface PdfMetadataInfo {
  title: string | null;
  author: string | null;
  creator: string | null;
  producer: string | null;
}

export interface FileInfo {
  path: string;
  size_bytes: number;
  pdf_version: string | null;
  pages: number | null;
  encrypted: boolean;
  metadata: PdfMetadataInfo;
  has_outlines: boolean;
  outline_count: number;
  first_page_size_pt: [number, number] | null;
}

export interface InfoReport {
  files: FileInfo[];
}

export interface MergeReport {
  inputs: string[];
  output: string;
  pages_total: number;
}

export interface SplitOutput {
  path: string;
  pages: number;
  page_range: [number, number] | null;
  title: string | null;
}

export interface SplitReport {
  input: string;
  input_pages: number;
  mode: string;
  outputs: SplitOutput[];
}

export interface PagesReport {
  action: string;
  input: string;
  output: string;
  input_pages: number;
  output_pages: number;
  pages_affected: number[];
  degrees?: number | null;
}

export interface CompressReport {
  input: string;
  output: string;
  preset: string;
  grayscale: boolean;
  size_before_bytes: number;
  size_after_bytes: number;
  saved_bytes: number;
  saved_percent: number;
  images_resampled: number;
  objects_deduped: number;
  already_optimal: boolean;
  dry_run: boolean;
}

export interface ThumbnailItem {
  page: number;
  width_px: number;
  height_px: number;
  data_url: string;
}

export interface ProgressEventPayload {
  op: string;
  done: number;
  total: number;
}

export interface CommandErrorPayload {
  code: string;
  message: string;
  hint: string | null;
}

export interface ToastNotice {
  title: string;
  subtitle: string;
  targetPath: string;
}
