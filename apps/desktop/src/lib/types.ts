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
