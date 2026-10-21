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

export type ConvertActionUi = 'to-images' | 'from-images' | 'to-markdown' | 'from-markdown';

export type ImageFormatUi = 'png' | 'jpg';

export type ConvertPageSizeUi = 'fit' | 'a4' | 'letter';

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

export interface ConvertedImagePage {
  page: number;
  path: string;
  width_px: number;
  height_px: number;
}

export interface PdfToImagesReport {
  action: 'to-images';
  input: string;
  output_dir: string;
  format: string;
  dpi: number;
  total_pages: number;
  outputs: ConvertedImagePage[];
  dry_run: boolean;
}

export interface ImagesToPdfReport {
  action: 'from-images';
  inputs: string[];
  output: string;
  page_size: string;
  margin_pt: number;
  pages: number;
  dry_run: boolean;
}

export interface PdfToMarkdownReport {
  action: 'to-markdown';
  input: string;
  output: string;
  total_pages: number;
  extracted_pages: number;
  char_count: number;
  line_count: number;
  dry_run: boolean;
}

export interface MarkdownToPdfReport {
  action: 'from-markdown';
  input: string;
  output: string;
  page_size: string;
  margin_pt: number;
  pages: number;
  headings_count: number;
  dry_run: boolean;
}

export type ConvertReport =
  | PdfToImagesReport
  | ImagesToPdfReport
  | PdfToMarkdownReport
  | MarkdownToPdfReport;

export type OverlayModeUi = 'watermark' | 'number';

export type WatermarkPositionUi =
  | 'diagonal'
  | 'center'
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right';

export type NumberPositionUi =
  | 'bottom-center'
  | 'bottom-right'
  | 'bottom-left'
  | 'top-center'
  | 'top-right'
  | 'top-left';

export interface WatermarkReport {
  input: string;
  output: string;
  text: string;
  position: string;
  font_size: number;
  opacity: number;
  rotation_deg: number;
  color: string;
  total_pages: number;
  pages_watermarked: number[];
  dry_run: boolean;
}

export interface NumberReport {
  input: string;
  output: string;
  format: string;
  position: string;
  start: number;
  font_size: number;
  opacity: number;
  color: string;
  total_pages: number;
  pages_numbered: number[];
  dry_run: boolean;
}

export type ProtectActionUi = 'protect' | 'unlock';

export interface ProtectReport {
  action: 'protect' | 'unlock';
  input: string;
  output: string;
  pages: number;
  encrypted: boolean;
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
