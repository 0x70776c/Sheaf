<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import {
    FolderOpen,
    X,
    AlertCircle,
    RotateCw,
    RotateCcw,
    ArrowUpDown,
    Lock,
    LockOpen,
    Settings,
  } from '@lucide/svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import DropZone from './lib/components/DropZone.svelte';
  import CommandPalette from './lib/components/CommandPalette.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import ThumbnailGrid from './lib/components/ThumbnailGrid.svelte';
  import MergeFileList from './lib/components/MergeFileList.svelte';
  import Toast from './lib/components/Toast.svelte';
  import InfoPanel from './lib/components/InfoPanel.svelte';
  import SettingsModal from './lib/components/SettingsModal.svelte';
  import { TOOLS, basename, formatBytes, getToolById } from './lib/tools';
  import { getInitialLanguage, saveLanguage, translations, type Language } from './lib/i18n';
  import {
    formatSelectedPagesToRanges,
    parseRangesToPageSet,
  } from './lib/selection';
  import {
    cancelActiveOperation,
    extractErrorMessage,
    fetchThumbnails,
    isTauriRuntime,
    onProgressEvent,
    openPathInSystem,
    openPdf,
    pickImageFiles,
    pickMarkdownFile,
    pickOutputDirectory,
    pickPdfFiles,
    pickSaveMarkdownPath,
    pickSavePdfPath,
    revealInFinder,
    runCompressCommand,
    runConvertCommand,
    runMergeCommand,
    runNumberCommand,
    runPagesCommand,
    runProtectCommand,
    runSplitCommand,
    runWatermarkCommand,
    previewOverlayCommand,
    startWindowDrag,
  } from './lib/tauri';
  import type {
    CommandErrorPayload,
    CompressPresetUi,
    ConvertActionUi,
    ConvertPageSizeUi,
    FileInfo,
    ImageFormatUi,
    NumberPositionUi,
    OverlayModeUi,
    PagesActionUi,
    ProgressEventPayload,
    ProtectActionUi,
    SplitModeUi,
    ThemePreference,
    ThumbnailItem,
    ToastNotice,
    ToolId,
    WatermarkPositionUi,
  } from './lib/types';

  let activeTool = $state<ToolId>('merge');
  let theme = $state<ThemePreference>('system');
  let lang = $state<Language>(getInitialLanguage());
  let paletteOpen = $state(false);
  let settingsDialogOpen = $state(false);
  let isDraggingOver = $state(false);

  let loadedFiles = $state<FileInfo[]>([]);
  let thumbnailsMap = $state<Map<number, ThumbnailItem>>(new Map());
  let selectedPages = $state<Set<number>>(new Set());
  let loadingThumbnails = $state(false);

  // Merge options
  let mergeOutPath = $state<string | null>(null);

  // Split options
  let splitMode = $state<SplitModeUi>('ranges');
  let splitRangesInput = $state('1');
  let splitCombine = $state(false);
  let splitEveryN = $state(2);
  let splitBookmarksLevel = $state(1);
  let splitOutDir = $state<string | null>(null);

  // Pages options
  let pagesAction = $state<PagesActionUi>('rotate');
  let pagesSpecInput = $state('');
  let rotateDegrees = $state<number>(90);
  let pagesOutPath = $state<string | null>(null);

  // Compress options
  let compressPreset = $state<CompressPresetUi>('medium');
  let compressGrayscale = $state(false);
  let compressOutPath = $state<string | null>(null);

  // Convert options
  let convertAction = $state<ConvertActionUi>('to-images');
  let convertFormat = $state<ImageFormatUi>('png');
  let convertDpi = $state<number>(150);
  let convertQuality = $state<number>(85);
  let convertPagesSpec = $state('');
  let convertPageSize = $state<ConvertPageSizeUi>('a4');
  let convertMarginPt = $state<number>(36);
  let convertOutPath = $state<string | null>(null);
  let convertImagePaths = $state<string[]>([]);
  let convertMarkdownPath = $state<string | null>(null);

  // Watermark & Number (Overlay) options
  let overlayMode = $state<OverlayModeUi>('watermark');
  let watermarkText = $state('CONFIDENTIAL');
  let watermarkPosition = $state<WatermarkPositionUi>('diagonal');
  let watermarkFontSize = $state<number>(48);
  let watermarkOpacity = $state<number>(0.25);
  let watermarkColor = $state('#6B7280');
  let watermarkPagesSpec = $state('');
  let watermarkOutPath = $state<string | null>(null);

  // Number options
  let numberFormat = $state('{n}');
  let numberPosition = $state<NumberPositionUi>('bottom-center');
  let numberStart = $state<number>(1);
  let numberFontSize = $state<number>(10);
  let numberOpacity = $state<number>(1.0);
  let numberColor = $state('#444444');
  let numberMargin = $state<number>(28);
  let numberPagesSpec = $state('');
  let numberOutPath = $state<string | null>(null);

  // Overlay Live Preview state
  let overlayPreviewItem = $state<ThumbnailItem | null>(null);
  let overlayPreviewLoading = $state(false);
  let overlayPreviewError = $state<string | null>(null);
  let previewReqSeq = 0;

  // Protect options
  let protectAction = $state<ProtectActionUi>('protect');
  let protectEnableProtection = $state(true);
  let protectPassword = $state('');
  let protectPasswordConfirm = $state('');
  let protectOwnerPassword = $state('');
  let protectAllowPrint = $state(true);
  let protectAllowCopy = $state(true);
  let protectAllowModify = $state(true);
  let protectOutPath = $state<string | null>(null);

  // Transient Session Password & Prompt State (never saved to localStorage / disk)
  let sessionPassword = $state<string | null>(null);
  let passwordDialogOpen = $state(false);
  let passwordPromptInput = $state('');
  let passwordPromptError = $state<string | null>(null);
  let passwordPromptAttempts = $state(0);
  let pendingEncryptedPaths = $state<string[]>([]);

  // Execution & feedback state
  let isRunning = $state(false);
  let progress = $state<ProgressEventPayload | null>(null);
  let errorBanner = $state<CommandErrorPayload | null>(null);
  let toastNotice = $state<ToastNotice | null>(null);

  let t = $derived(translations[lang]);
  let currentToolMeta = $derived(getToolById(activeTool, lang));
  let primaryFile = $derived<FileInfo | undefined>(loadedFiles[0]);
  let primaryTotalPages = $derived(primaryFile?.pages ?? 0);
  let isReadOnly = $derived(
    Boolean(
      primaryFile?.encrypted &&
        primaryFile.encryption?.permissions &&
        (!primaryFile.encryption.permissions.modify || !primaryFile.encryption.permissions.copy),
    ),
  );
  let hasWorkspaceInput = $derived(
    loadedFiles.length > 0 ||
      (activeTool === 'convert' &&
        (convertImagePaths.length > 0 || convertMarkdownPath !== null)),
  );

  $effect(() => {
    const root = document.documentElement;
    if (theme === 'system') {
      root.removeAttribute('data-theme');
    } else {
      root.setAttribute('data-theme', theme);
    }
  });

  $effect(() => {
    if (activeTool !== 'watermark' || !primaryFile || primaryFile.encrypted) {
      overlayPreviewItem = null;
      return;
    }
    const currentFile = primaryFile.path;
    const total = primaryTotalPages;
    const currentMode = overlayMode;
    const wmText = watermarkText;
    const wmPos = watermarkPosition;
    const wmSize = watermarkFontSize;
    const wmOp = watermarkOpacity;
    const wmCol = watermarkColor;
    const numFmt = numberFormat;
    const numPos = numberPosition;
    const numStart = numberStart;
    const numSize = numberFontSize;
    const numOp = numberOpacity;
    const numCol = numberColor;
    const numMargin = numberMargin;

    const targetPage = selectedPages.size > 0 ? Math.min(...selectedPages) : 1;

    const reqId = ++previewReqSeq;
    overlayPreviewLoading = true;
    overlayPreviewError = null;

    const timer = setTimeout(async () => {
      try {
        const item = await previewOverlayCommand({
          input: currentFile,
          page: targetPage,
          totalPages: total,
          mode: currentMode,
          text: currentMode === 'number' ? numFmt : wmText,
          position: currentMode === 'number' ? numPos : wmPos,
          start: numStart,
          fontSize: currentMode === 'number' ? numSize : wmSize,
          opacity: currentMode === 'number' ? numOp : wmOp,
          color: currentMode === 'number' ? numCol : wmCol,
          margin: currentMode === 'number' ? numMargin : 36.0,
          widthPx: 260,
        });
        if (reqId === previewReqSeq) {
          overlayPreviewItem = item;
          overlayPreviewLoading = false;
        }
      } catch (err) {
        if (reqId === previewReqSeq) {
          overlayPreviewError = (err as Error)?.message ?? 'Preview unavailable';
          overlayPreviewLoading = false;
        }
      }
    }, 300);

    return () => {
      clearTimeout(timer);
    };
  });

  async function loadThumbnailsUpTo(file: FileInfo, maxPage: number) {
    if (!file.pages || file.pages <= 0) return;
    if (file.encrypted && !sessionPassword) return;
    const limit = Math.min(file.pages, maxPage);
    const missing: number[] = [];
    for (let p = 1; p <= limit; p++) {
      if (!thumbnailsMap.has(p)) {
        missing.push(p);
      }
    }
    if (missing.length === 0) return;

    loadingThumbnails = true;
    try {
      const items = await fetchThumbnails(file.path, missing, 160, sessionPassword);
      const next = new Map(thumbnailsMap);
      for (const item of items) {
        next.set(item.page, item);
      }
      thumbnailsMap = next;
    } catch (err) {
      errorBanner = extractErrorMessage(err);
    } finally {
      loadingThumbnails = false;
    }
  }

  async function loadPaths(paths: string[], append = false) {
    const imgExts = ['.png', '.jpg', '.jpeg'];
    const mdExts = ['.md', '.markdown'];

    const imageFiles = paths.filter((p) =>
      imgExts.some((ext) => p.toLowerCase().endsWith(ext)),
    );
    const mdFiles = paths.filter((p) =>
      mdExts.some((ext) => p.toLowerCase().endsWith(ext)),
    );
    const pdfPaths = paths.filter((p) => p.toLowerCase().endsWith('.pdf'));

    if (imageFiles.length > 0 && pdfPaths.length === 0) {
      activeTool = 'convert';
      convertAction = 'from-images';
      convertImagePaths = append
        ? [...convertImagePaths, ...imageFiles]
        : imageFiles;
      errorBanner = null;
      return;
    }

    if (mdFiles.length > 0 && pdfPaths.length === 0) {
      activeTool = 'convert';
      convertAction = 'from-markdown';
      convertMarkdownPath = mdFiles[0];
      errorBanner = null;
      return;
    }

    if (pdfPaths.length === 0) {
      errorBanner = {
        code: 'invalid_selection',
        message: 'Please select a .pdf, .png, .jpg, or .md file.',
        hint: null,
      };
      return;
    }

    errorBanner = null;
    if (!isTauriRuntime()) {
      return;
    }

    try {
      const report = await openPdf(pdfPaths, sessionPassword);
      if (append && activeTool === 'merge') {
        loadedFiles = [...loadedFiles, ...report.files];
      } else {
        const first = report.files[0];
        if (first?.encrypted && !sessionPassword) {
          // Encrypted file opened without password -> trigger modal prompt
          pendingEncryptedPaths = pdfPaths;
          passwordDialogOpen = true;
          passwordPromptInput = '';
          passwordPromptError = null;
          passwordPromptAttempts = 0;
          return;
        }

        loadedFiles = report.files;
        thumbnailsMap = new Map();
        selectedPages = new Set();
        if (first) {
          const total = first.pages ?? 1;
          splitRangesInput = total > 1 ? `1-${Math.min(3, total)}` : '1';
          pagesSpecInput = '';
          convertPagesSpec = '';
          watermarkPagesSpec = '';
          if (first.encrypted) {
            protectAction = 'unlock';
          }
          await loadThumbnailsUpTo(first, 20);
        }
      }
    } catch (err) {
      const errMsg = extractErrorMessage(err);
      if (errMsg.code === 'encrypted' || errMsg.code === 'wrong_password') {
        pendingEncryptedPaths = pdfPaths;
        passwordDialogOpen = true;
        passwordPromptInput = '';
        passwordPromptError = null;
        passwordPromptAttempts = 0;
      } else {
        errorBanner = errMsg;
      }
    }
  }

  async function handlePasswordSubmit() {
    if (!passwordPromptInput.trim() || pendingEncryptedPaths.length === 0) return;
    try {
      const pw = passwordPromptInput;
      const report = await openPdf(pendingEncryptedPaths, pw);
      sessionPassword = pw;
      passwordDialogOpen = false;
      passwordPromptError = null;
      passwordPromptAttempts = 0;

      loadedFiles = report.files;
      thumbnailsMap = new Map();
      selectedPages = new Set();
      const first = report.files[0];
      if (first) {
        const total = first.pages ?? 1;
        splitRangesInput = total > 1 ? `1-${Math.min(3, total)}` : '1';
        pagesSpecInput = '';
        convertPagesSpec = '';
        watermarkPagesSpec = '';
        if (first.encrypted) {
          protectAction = 'unlock';
        }
        await loadThumbnailsUpTo(first, 20);
      }
    } catch (err) {
      passwordPromptAttempts += 1;
      passwordPromptInput = '';
      passwordPromptError = 'Falsches Passwort. Bitte erneut versuchen.';
    }
  }

  function handlePasswordCancel() {
    passwordDialogOpen = false;
    pendingEncryptedPaths = [];
    passwordPromptInput = '';
    passwordPromptError = null;
    passwordPromptAttempts = 0;
  }

  async function handleChooseFiles(append = false) {
    if (activeTool === 'convert' && convertAction === 'from-images') {
      const imgs = await pickImageFiles(true);
      if (imgs.length > 0) {
        convertImagePaths = append
          ? [...convertImagePaths, ...imgs]
          : imgs;
      }
      return;
    }
    if (activeTool === 'convert' && convertAction === 'from-markdown') {
      const md = await pickMarkdownFile();
      if (md) {
        convertMarkdownPath = md;
      }
      return;
    }
    const allowMultiple = activeTool === 'merge';
    const selected = await pickPdfFiles(allowMultiple);
    if (selected.length > 0) {
      await loadPaths(selected, append);
    }
  }

  function handleClearFiles() {
    loadedFiles = [];
    convertImagePaths = [];
    convertMarkdownPath = null;
    thumbnailsMap = new Map();
    selectedPages = new Set();
    errorBanner = null;
    sessionPassword = null;
    passwordDialogOpen = false;
    pendingEncryptedPaths = [];
  }

  function handleGridSelectionChange(nextSelected: Set<number>) {
    selectedPages = nextSelected;
    const formatted = formatSelectedPagesToRanges(nextSelected);
    if (activeTool === 'split' && splitMode === 'ranges' && formatted) {
      splitRangesInput = formatted;
    } else if (activeTool === 'pages' && pagesAction !== 'reorder') {
      pagesSpecInput = formatted;
    } else if (
      activeTool === 'convert' &&
      (convertAction === 'to-images' || convertAction === 'to-markdown')
    ) {
      convertPagesSpec = formatted;
    } else if (activeTool === 'watermark') {
      if (overlayMode === 'number') {
        numberPagesSpec = formatted;
      } else {
        watermarkPagesSpec = formatted;
      }
    }
  }

  function handleRangesInputEdit(
    value: string,
    target: 'split' | 'pages' | 'convert' | 'watermark' | 'number',
  ) {
    if (target === 'split') {
      splitRangesInput = value;
    } else if (target === 'pages') {
      pagesSpecInput = value;
    } else if (target === 'convert') {
      convertPagesSpec = value;
    } else if (target === 'number') {
      numberPagesSpec = value;
    } else {
      watermarkPagesSpec = value;
    }
    if (primaryTotalPages > 0) {
      selectedPages = parseRangesToPageSet(value, primaryTotalPages);
    }
  }

  function handleReversePagesOrder() {
    if (primaryTotalPages <= 0) return;
    const reversed = Array.from(
      { length: primaryTotalPages },
      (_, idx) => primaryTotalPages - idx,
    ).join(',');
    pagesSpecInput = reversed;
  }

  function handleMoveMergeFile(index: number, direction: -1 | 1) {
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= loadedFiles.length) return;
    const copy = [...loadedFiles];
    const [item] = copy.splice(index, 1);
    copy.splice(targetIndex, 0, item);
    loadedFiles = copy;
  }

  function handleRemoveMergeFile(index: number) {
    loadedFiles = loadedFiles.filter((_, idx) => idx !== index);
  }

  async function handleRunPrimaryAction() {
    if (isRunning || !hasWorkspaceInput) return;
    errorBanner = null;
    toastNotice = null;
    isRunning = true;

    try {
      if (activeTool === 'merge') {
        const inputs = loadedFiles.map((f) => f.path);
        const rep = await runMergeCommand(inputs, mergeOutPath, true);
        toastNotice = {
          title: `Merged ${rep.inputs.length} files (${rep.pages_total} pages)`,
          subtitle: basename(rep.output),
          targetPath: rep.output,
        };
      } else if (activeTool === 'split' && primaryFile) {
        const rep = await runSplitCommand({
          input: primaryFile.path,
          mode: splitMode,
          ranges: splitMode === 'ranges' ? splitRangesInput : null,
          combine: splitMode === 'ranges' ? splitCombine : false,
          every: splitMode === 'every' ? splitEveryN : null,
          bookmarksLevel: splitMode === 'bookmarks' ? splitBookmarksLevel : null,
          outDir: splitOutDir,
          force: true,
        });
        const firstOutput = rep.outputs[0]?.path ?? primaryFile.path;
        toastNotice = {
          title: `Split into ${rep.outputs.length} ${rep.outputs.length === 1 ? 'file' : 'files'}`,
          subtitle: basename(firstOutput),
          targetPath: firstOutput,
        };
      } else if (activeTool === 'pages' && primaryFile) {
        const rep = await runPagesCommand({
          input: primaryFile.path,
          action: pagesAction,
          pages: pagesSpecInput.trim() || null,
          degrees: pagesAction === 'rotate' ? rotateDegrees : null,
          out: pagesOutPath,
          force: true,
        });
        toastNotice = {
          title: `Pages ${rep.action} complete (${rep.output_pages} pages)`,
          subtitle: basename(rep.output),
          targetPath: rep.output,
        };
        // Reload thumbnails if operating on a document
        const updatedInfo = await openPdf([primaryFile.path]);
        loadedFiles = updatedInfo.files;
      } else if (activeTool === 'compress' && primaryFile) {
        const rep = await runCompressCommand({
          input: primaryFile.path,
          preset: compressPreset,
          grayscale: compressGrayscale,
          out: compressOutPath,
          force: true,
        });
        toastNotice = rep.already_optimal
          ? {
              title: 'Document is already optimal',
              subtitle: `${formatBytes(rep.size_before_bytes)} · ${basename(rep.output)}`,
              targetPath: rep.output,
            }
          : {
              title: `Saved ${rep.saved_percent.toFixed(1)}% (${formatBytes(rep.size_before_bytes)} → ${formatBytes(rep.size_after_bytes)})`,
              subtitle: basename(rep.output),
              targetPath: rep.output,
            };
      } else if (activeTool === 'convert') {
        const inputs =
          convertAction === 'from-images'
            ? convertImagePaths
            : convertAction === 'from-markdown'
              ? convertMarkdownPath
                ? [convertMarkdownPath]
                : []
              : primaryFile
                ? [primaryFile.path]
                : [];

        const rep = await runConvertCommand({
          action: convertAction,
          inputs,
          format: convertAction === 'to-images' ? convertFormat : null,
          dpi: convertAction === 'to-images' ? convertDpi : null,
          quality: convertAction === 'to-images' ? convertQuality : null,
          pages:
            convertAction === 'to-images' || convertAction === 'to-markdown'
              ? convertPagesSpec.trim() || null
              : null,
          pageSize:
            convertAction === 'from-images' || convertAction === 'from-markdown'
              ? convertPageSize
              : null,
          marginPt:
            convertAction === 'from-images' || convertAction === 'from-markdown'
              ? convertMarginPt
              : null,
          out: convertOutPath,
          force: true,
        });

        if (rep.action === 'to-images') {
          const target = rep.outputs[0]?.path ?? rep.output_dir;
          toastNotice = {
            title: `Exported ${rep.outputs.length} ${rep.format.toUpperCase()} ${rep.outputs.length === 1 ? 'image' : 'images'}`,
            subtitle: basename(rep.output_dir),
            targetPath: target,
          };
        } else if (rep.action === 'from-images') {
          toastNotice = {
            title: `Created ${rep.pages}-page PDF from images`,
            subtitle: basename(rep.output),
            targetPath: rep.output,
          };
        } else if (rep.action === 'to-markdown') {
          toastNotice = {
            title: `Extracted Markdown (${rep.line_count} lines)`,
            subtitle: basename(rep.output),
            targetPath: rep.output,
          };
        } else {
          toastNotice = {
            title: `Rendered ${rep.pages}-page PDF from Markdown`,
            subtitle: basename(rep.output),
            targetPath: rep.output,
          };
        }
      } else if (activeTool === 'watermark' && primaryFile) {
        if (overlayMode === 'number') {
          const rep = await runNumberCommand({
            input: primaryFile.path,
            format: numberFormat,
            position: numberPosition,
            start: numberStart,
            fontSize: numberFontSize,
            opacity: numberOpacity,
            color: numberColor,
            margin: numberMargin,
            pages: numberPagesSpec.trim() || null,
            out: numberOutPath,
            force: true,
          });
          toastNotice = {
            title: `Numbered ${rep.pages_numbered.length} ${rep.pages_numbered.length === 1 ? 'page' : 'pages'}`,
            subtitle: basename(rep.output),
            targetPath: rep.output,
          };
        } else {
          const rep = await runWatermarkCommand({
            input: primaryFile.path,
            text: watermarkText,
            position: watermarkPosition,
            fontSize: watermarkFontSize,
            opacity: watermarkOpacity,
            color: watermarkColor,
            pages: watermarkPagesSpec.trim() || null,
            out: watermarkOutPath,
            force: true,
          });
          toastNotice = {
            title: `Watermarked ${rep.pages_watermarked.length} ${rep.pages_watermarked.length === 1 ? 'page' : 'pages'}`,
            subtitle: basename(rep.output),
            targetPath: rep.output,
          };
        }
      } else if (activeTool === 'protect' && primaryFile) {
        if (protectAction === 'protect') {
          if (!protectPassword) {
            errorBanner = {
              code: 'invalid_selection',
              message: 'Bitte ein Passwort eingeben.',
              hint: null,
            };
            return;
          }
          if (protectPassword !== protectPasswordConfirm) {
            errorBanner = {
              code: 'password_mismatch',
              message: 'Passwörter stimmen nicht überein.',
              hint: 'Bitte überprüfen Sie die Wiederholung des Passworts.',
            };
            return;
          }
        } else if (protectAction === 'unlock') {
          if (!protectPassword && sessionPassword) {
            protectPassword = sessionPassword;
          }
          if (!protectPassword) {
            errorBanner = {
              code: 'invalid_selection',
              message: 'Bitte das Passwort zum Entsperren eingeben.',
              hint: null,
            };
            return;
          }
        }
        const rep = await runProtectCommand({
          action: protectAction,
          input: primaryFile.path,
          password: protectPassword,
          ownerPassword:
            protectAction === 'protect' && protectOwnerPassword.trim()
              ? protectOwnerPassword.trim()
              : null,
          allowPrint: protectAllowPrint,
          allowCopy: protectAllowCopy,
          allowModify: protectAllowModify,
          out: protectOutPath,
          force: true,
        });
        toastNotice = {
          title:
            rep.action === 'protect'
              ? `PDF geschützt (${rep.pages} Seiten)`
              : `PDF entsperrt (${rep.pages} Seiten)`,
          subtitle: basename(rep.output),
          targetPath: rep.output,
        };
      }
    } catch (err) {
      errorBanner = extractErrorMessage(err);
    } finally {
      isRunning = false;
      progress = null;
    }
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (passwordDialogOpen) {
      if (event.key === 'Escape') {
        event.preventDefault();
        handlePasswordCancel();
        return;
      }
      if (event.key === 'Enter') {
        event.preventDefault();
        void handlePasswordSubmit();
        return;
      }
    }

    const isCmd = event.metaKey || event.ctrlKey;
    const targetTag = (event.target as HTMLElement | null)?.tagName;
    const isInputFocused =
      targetTag === 'INPUT' || targetTag === 'TEXTAREA' || targetTag === 'SELECT';

    if (isCmd && event.key === ',') {
      event.preventDefault();
      settingsDialogOpen = !settingsDialogOpen;
      return;
    }
    if (isCmd && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      paletteOpen = !paletteOpen;
      return;
    }
    if (isCmd && event.key.toLowerCase() === 'o') {
      event.preventDefault();
      paletteOpen = false;
      void handleChooseFiles(false);
      return;
    }
    if (
      isCmd &&
      event.key.toLowerCase() === 'a' &&
      !isInputFocused &&
      !paletteOpen &&
      activeTool !== 'merge' &&
      primaryTotalPages > 0
    ) {
      event.preventDefault();
      if (selectedPages.size === primaryTotalPages) {
        handleGridSelectionChange(new Set());
      } else {
        const all = new Set(
          Array.from({ length: primaryTotalPages }, (_, i) => i + 1),
        );
        handleGridSelectionChange(all);
      }
      return;
    }
    if (isCmd && /^[1-8]$/.test(event.key)) {
      const idx = Number(event.key) - 1;
      if (TOOLS[idx]) {
        event.preventDefault();
        activeTool = TOOLS[idx].id;
      }
      return;
    }
    if (event.key === 'Escape' && !paletteOpen) {
      if (errorBanner) {
        errorBanner = null;
      } else if (toastNotice) {
        toastNotice = null;
      }
    }
  }

  onMount(() => {
    let unlistenDragDrop: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;

    if (isTauriRuntime()) {
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          if (event.payload.type === 'enter' || event.payload.type === 'over') {
            isDraggingOver = true;
          } else if (event.payload.type === 'leave') {
            isDraggingOver = false;
          } else if (event.payload.type === 'drop') {
            isDraggingOver = false;
            if (event.payload.paths.length > 0) {
              const appendInMerge =
                activeTool === 'merge' && loadedFiles.length > 0;
              void loadPaths(event.payload.paths, appendInMerge);
            }
          }
        })
        .then((fn) => {
          unlistenDragDrop = fn;
        });

      void onProgressEvent((payload) => {
        if (payload.done >= payload.total) {
          progress = null;
        } else {
          progress = payload;
        }
      }).then((fn) => {
        unlistenProgress = fn;
      });
    }

    return () => {
      unlistenDragDrop?.();
      unlistenProgress?.();
    };
  });
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="app-shell">
  <div class="workspace-row">
    <Sidebar
      {activeTool}
      {lang}
      onSelectTool={(id) => {
        activeTool = id;
      }}
      onOpenCommandPalette={() => {
        paletteOpen = true;
      }}
      onOpenSettings={() => {
        settingsDialogOpen = true;
      }}
    />

    <main class="main-column" aria-label={`${currentToolMeta.label} Workspace`}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <header
        class="titlebar drag-region"
        data-tauri-drag-region
        onmousedown={startWindowDrag}
        onpointerdown={startWindowDrag}
      >
        <div
          class="titlebar-title drag-region"
          data-tauri-drag-region
          onmousedown={startWindowDrag}
          onpointerdown={startWindowDrag}
        >
          <span class="app-name" data-tauri-drag-region>Sheaf</span>
          <span class="title-sep" data-tauri-drag-region aria-hidden="true">·</span>
          <span class="tool-title" data-tauri-drag-region>{currentToolMeta.label}</span>
          {#if primaryFile}
            <span class="title-sep" data-tauri-drag-region aria-hidden="true">·</span>
            {#if !primaryFile.encrypted}
              <span class="lock-status-badge unencrypted drag-region" data-tauri-drag-region title={t.unencryptedDesc}>
                <LockOpen size={12} strokeWidth={2} data-tauri-drag-region />
                <span data-tauri-drag-region>{t.unencrypted}</span>
              </span>
            {:else if isReadOnly}
              <span class="lock-status-badge readonly drag-region" data-tauri-drag-region title={t.readOnlyDesc}>
                <Lock size={12} strokeWidth={2} data-tauri-drag-region />
                <span data-tauri-drag-region>{t.readOnly}</span>
              </span>
            {:else if sessionPassword}
              <span class="lock-status-badge unlocked drag-region" data-tauri-drag-region title={t.unlockedDesc}>
                <Lock size={12} strokeWidth={2} data-tauri-drag-region />
                <span data-tauri-drag-region>{t.unlocked}</span>
              </span>
            {:else}
              <span class="lock-status-badge locked drag-region" data-tauri-drag-region title={t.protectedDesc}>
                <Lock size={12} strokeWidth={2} data-tauri-drag-region />
                <span data-tauri-drag-region>{t.protected}</span>
              </span>
            {/if}
          {/if}
        </div>

        <div class="titlebar-actions no-drag">
          {#if hasWorkspaceInput}
            <button
              type="button"
              class="ghost-btn"
              onclick={() => void handleChooseFiles(false)}
              aria-label="Open file (Command+O)"
            >
              <FolderOpen size={14} strokeWidth={1.5} aria-hidden="true" />
              <span>{t.openFile}</span>
            </button>
            <button
              type="button"
              class="ghost-btn"
              onclick={handleClearFiles}
              aria-label="Clear loaded files"
            >
              <X size={14} strokeWidth={1.5} aria-hidden="true" />
              <span>{t.clearFiles}</span>
            </button>
          {/if}
          <button
            type="button"
            class="ghost-btn"
            onclick={() => {
              settingsDialogOpen = true;
            }}
            aria-label={t.settings}
            title={t.settings}
          >
            <Settings size={14} strokeWidth={1.5} aria-hidden="true" />
            <span>{t.settings}</span>
          </button>
        </div>
      </header>

      {#if errorBanner}
        <div class="error-banner" role="alert">
          <AlertCircle size={16} strokeWidth={1.5} aria-hidden="true" />
          <div class="error-text">
            <strong>{errorBanner.message}</strong>
            {#if errorBanner.hint}
              <span>{errorBanner.hint}</span>
            {/if}
          </div>
          <button
            type="button"
            class="error-dismiss"
            aria-label="Dismiss error"
            onclick={() => {
              errorBanner = null;
            }}
          >
            <X size={14} strokeWidth={1.5} aria-hidden="true" />
          </button>
        </div>
      {/if}

      {#if !hasWorkspaceInput}
        <DropZone
          tool={currentToolMeta}
          {isDraggingOver}
          onChooseFiles={() => void handleChooseFiles(false)}
          onDropFiles={(paths) => {
            void loadPaths(paths, false);
          }}
          onDragStateChange={(hovering) => {
            isDraggingOver = hovering;
          }}
        />
      {:else}
        <div class="loaded-workspace">
          {#if activeTool === 'merge'}
            <MergeFileList
              files={loadedFiles}
              onMoveFile={handleMoveMergeFile}
              onRemoveFile={handleRemoveMergeFile}
              onAddMoreFiles={() => void handleChooseFiles(true)}
            />
          {:else if primaryFile}
            <ThumbnailGrid
              totalPages={primaryTotalPages}
              thumbnails={thumbnailsMap}
              {selectedPages}
              loading={loadingThumbnails}
              onSelectionChange={handleGridSelectionChange}
              onRequestMoreThumbnails={(upTo) => {
                if (primaryFile) {
                  void loadThumbnailsUpTo(primaryFile, upTo);
                }
              }}
            />
          {:else}
            <div class="convert-source-pane">
              {#if convertAction === 'from-images'}
                <div class="source-card">
                  <h3 class="source-card-title">
                    Images to Combine ({convertImagePaths.length})
                  </h3>
                  <ul class="source-path-list">
                    {#each convertImagePaths as imgPath, idx (imgPath + idx)}
                      <li class="source-path-item">
                        <span class="source-index">{idx + 1}.</span>
                        <span class="source-name">{basename(imgPath)}</span>
                      </li>
                    {/each}
                  </ul>
                  <button
                    type="button"
                    class="small-btn"
                    onclick={() => void handleChooseFiles(true)}
                  >
                    Add Images…
                  </button>
                </div>
              {:else if convertAction === 'from-markdown' && convertMarkdownPath}
                <div class="source-card">
                  <h3 class="source-card-title">Markdown Source</h3>
                  <p class="source-name">{basename(convertMarkdownPath)}</p>
                  <button
                    type="button"
                    class="small-btn"
                    onclick={() => void handleChooseFiles(false)}
                  >
                    Change File…
                  </button>
                </div>
              {/if}
            </div>
          {/if}

          <aside
            class="options-panel"
            aria-label={`${currentToolMeta.label} Options`}
          >
            <div class="options-body">
              <div class="panel-header">
                <h2 class="panel-heading">{currentToolMeta.label}</h2>
                <p class="panel-description">{currentToolMeta.description}</p>
              </div>

              {#if activeTool === 'merge'}
                <div class="option-group">
                  <span class="field-label">Files to Merge</span>
                  <div class="summary-pill">
                    {loadedFiles.length} {loadedFiles.length === 1 ? 'file' : 'files'} ·
                    {loadedFiles.reduce((acc, f) => acc + (f.pages ?? 0), 0)} total pages
                  </div>
                </div>

                <div class="option-group">
                  <span class="field-label">Output File</span>
                  <div class="path-picker-row">
                    <span class="path-preview">
                      {mergeOutPath
                        ? basename(mergeOutPath)
                        : `${basename(loadedFiles[0].path).replace(/\.pdf$/i, '')}_merged.pdf`}
                    </span>
                    <button
                      type="button"
                      class="small-btn"
                      onclick={async () => {
                        const chosen = await pickSavePdfPath('merged.pdf');
                        if (chosen) mergeOutPath = chosen;
                      }}
                    >
                      Save As…
                    </button>
                  </div>
                </div>
              {:else if activeTool === 'split' && primaryFile}
                <div class="option-group">
                  <span class="field-label">Split Mode</span>
                  <div class="segmented-control" role="radiogroup" aria-label="Split Mode">
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={splitMode === 'ranges'}
                      onclick={() => {
                        splitMode = 'ranges';
                      }}
                    >
                      Ranges
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={splitMode === 'every'}
                      onclick={() => {
                        splitMode = 'every';
                      }}
                    >
                      Every N
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={splitMode === 'bookmarks'}
                      onclick={() => {
                        splitMode = 'bookmarks';
                      }}
                    >
                      Bookmarks
                    </button>
                  </div>
                </div>

                {#if splitMode === 'ranges'}
                  <div class="option-group">
                    <label class="field-label" for="split-ranges-input">
                      Page Ranges (e.g. 1-3, 5, 8-)
                    </label>
                    <input
                      id="split-ranges-input"
                      type="text"
                      class="text-input"
                      value={splitRangesInput}
                      oninput={(e) =>
                        handleRangesInputEdit(e.currentTarget.value, 'split')}
                      placeholder="1-3, 5, 8-"
                    />
                  </div>

                  <label class="checkbox-row">
                    <input
                      type="checkbox"
                      checked={splitCombine}
                      onchange={(e) => {
                        splitCombine = e.currentTarget.checked;
                      }}
                    />
                    <span>Combine ranges into one PDF</span>
                  </label>
                {:else if splitMode === 'every'}
                  <div class="option-group">
                    <label class="field-label" for="split-every-input">
                      Pages per File (N)
                    </label>
                    <input
                      id="split-every-input"
                      type="number"
                      min="1"
                      max={Math.max(1, primaryTotalPages)}
                      class="text-input"
                      value={splitEveryN}
                      oninput={(e) => {
                        splitEveryN = Math.max(1, Number(e.currentTarget.value) || 1);
                      }}
                    />
                  </div>
                {:else}
                  <div class="option-group">
                    <label class="field-label" for="split-bm-level">
                      Bookmark Hierarchy Level
                    </label>
                    <select
                      id="split-bm-level"
                      class="text-input"
                      value={splitBookmarksLevel}
                      onchange={(e) => {
                        splitBookmarksLevel = Number(e.currentTarget.value) || 1;
                      }}
                    >
                      <option value={1}>Level 1 (Top-level chapters)</option>
                      <option value={2}>Level 2 (Sections)</option>
                      <option value={3}>Level 3 (Subsections)</option>
                    </select>
                  </div>
                {/if}

                <div class="option-group">
                  <span class="field-label">Output Directory</span>
                  <div class="path-picker-row">
                    <span class="path-preview">
                      {splitOutDir ? basename(splitOutDir) : 'Same as source'}
                    </span>
                    <button
                      type="button"
                      class="small-btn"
                      onclick={async () => {
                        const dir = await pickOutputDirectory();
                        if (dir) splitOutDir = dir;
                      }}
                    >
                      Folder…
                    </button>
                  </div>
                </div>
              {:else if activeTool === 'pages' && primaryFile}
                <div class="option-group">
                  <span class="field-label">Action</span>
                  <div class="segmented-control" role="radiogroup" aria-label="Page Action">
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={pagesAction === 'rotate'}
                      onclick={() => {
                        pagesAction = 'rotate';
                      }}
                    >
                      Rotate
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={pagesAction === 'delete'}
                      onclick={() => {
                        pagesAction = 'delete';
                      }}
                    >
                      Delete
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={pagesAction === 'reorder'}
                      onclick={() => {
                        pagesAction = 'reorder';
                      }}
                    >
                      Reorder
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={pagesAction === 'extract'}
                      onclick={() => {
                        pagesAction = 'extract';
                      }}
                    >
                      Extract
                    </button>
                  </div>
                </div>

                {#if pagesAction === 'rotate'}
                  <div class="option-group">
                    <span class="field-label">Rotation Angle</span>
                    <div class="segmented-control">
                      <button
                        type="button"
                        class="seg-btn"
                        class:active={rotateDegrees === -90}
                        onclick={() => {
                          rotateDegrees = -90;
                        }}
                      >
                        <RotateCcw size={13} strokeWidth={1.5} aria-hidden="true" />
                        <span>90° CCW</span>
                      </button>
                      <button
                        type="button"
                        class="seg-btn"
                        class:active={rotateDegrees === 90}
                        onclick={() => {
                          rotateDegrees = 90;
                        }}
                      >
                        <RotateCw size={13} strokeWidth={1.5} aria-hidden="true" />
                        <span>90° CW</span>
                      </button>
                      <button
                        type="button"
                        class="seg-btn"
                        class:active={rotateDegrees === 180}
                        onclick={() => {
                          rotateDegrees = 180;
                        }}
                      >
                        <span>180°</span>
                      </button>
                    </div>
                  </div>
                {/if}

                <div class="option-group">
                  <div class="label-row">
                    <label class="field-label" for="pages-spec-input">
                      {pagesAction === 'reorder'
                        ? 'New Page Order (e.g. 3,1,2,4-)'
                        : pagesAction === 'rotate'
                          ? 'Target Pages (empty = all pages)'
                          : 'Target Pages (e.g. 1-3, 5)'}
                    </label>
                    {#if pagesAction === 'reorder'}
                      <button
                        type="button"
                        class="inline-link-btn"
                        onclick={handleReversePagesOrder}
                      >
                        <ArrowUpDown size={11} strokeWidth={1.5} aria-hidden="true" />
                        <span>Reverse</span>
                      </button>
                    {/if}
                  </div>
                  <input
                    id="pages-spec-input"
                    type="text"
                    class="text-input"
                    value={pagesSpecInput}
                    oninput={(e) =>
                      handleRangesInputEdit(e.currentTarget.value, 'pages')}
                    placeholder={pagesAction === 'rotate'
                      ? 'All pages'
                      : pagesAction === 'reorder'
                        ? '3, 1, 2, 4-'
                        : '1-3, 5'}
                  />
                </div>
              {:else if activeTool === 'compress' && primaryFile}
                <div class="option-group">
                  <span class="field-label">Compression Preset</span>
                  <div class="segmented-control" role="radiogroup" aria-label="Compression Preset">
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={compressPreset === 'low'}
                      onclick={() => {
                        compressPreset = 'low';
                      }}
                    >
                      Low
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={compressPreset === 'medium'}
                      onclick={() => {
                        compressPreset = 'medium';
                      }}
                    >
                      Medium
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={compressPreset === 'high'}
                      onclick={() => {
                        compressPreset = 'high';
                      }}
                    >
                      High
                    </button>
                  </div>
                </div>

                <div class="summary-pill">
                  {#if compressPreset === 'low'}
                    220 DPI · JPEG 85 · Structure optimization
                  {:else if compressPreset === 'medium'}
                    150 DPI · JPEG 70 · Deduplicate & resample
                  {:else}
                    100 DPI · JPEG 55 · Maximum size reduction
                  {/if}
                </div>

                <label class="checkbox-row">
                  <input
                    type="checkbox"
                    checked={compressGrayscale}
                    onchange={(e) => {
                      compressGrayscale = e.currentTarget.checked;
                    }}
                  />
                  <span>Convert images to grayscale</span>
                </label>

                <div class="option-group">
                  <span class="field-label">Original Size</span>
                  <div class="summary-pill">{formatBytes(primaryFile.size_bytes)}</div>
                </div>
              {:else if activeTool === 'convert'}
                <div class="option-group">
                  <label class="field-label" for="convert-mode-select">
                    Conversion Mode
                  </label>
                  <select
                    id="convert-mode-select"
                    class="text-input"
                    value={convertAction}
                    onchange={(e) => {
                      convertAction = e.currentTarget.value as ConvertActionUi;
                      convertOutPath = null;
                    }}
                  >
                    <option value="to-images">PDF → Images (PNG / JPG)</option>
                    <option value="from-images">Images → PDF</option>
                    <option value="to-markdown">PDF → Markdown (.md)</option>
                    <option value="from-markdown">Markdown (.md) → PDF</option>
                  </select>
                </div>

                {#if convertAction === 'to-images'}
                  <div class="option-group">
                    <span class="field-label">Image Format</span>
                    <div class="segmented-control">
                      <button
                        type="button"
                        class="seg-btn"
                        class:active={convertFormat === 'png'}
                        onclick={() => {
                          convertFormat = 'png';
                        }}
                      >
                        PNG
                      </button>
                      <button
                        type="button"
                        class="seg-btn"
                        class:active={convertFormat === 'jpg'}
                        onclick={() => {
                          convertFormat = 'jpg';
                        }}
                      >
                        JPG
                      </button>
                    </div>
                  </div>

                  <div class="option-group">
                    <span class="field-label">Resolution (DPI)</span>
                    <div class="segmented-control">
                      {#each [72, 150, 300] as dpiVal (dpiVal)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={convertDpi === dpiVal}
                          onclick={() => {
                            convertDpi = dpiVal;
                          }}
                        >
                          {dpiVal} DPI
                        </button>
                      {/each}
                    </div>
                  </div>

                  {#if convertFormat === 'jpg'}
                    <div class="option-group">
                      <label class="field-label" for="convert-jpg-quality">
                        JPEG Quality ({convertQuality}%)
                      </label>
                      <input
                        id="convert-jpg-quality"
                        type="range"
                        min="40"
                        max="100"
                        step="5"
                        value={convertQuality}
                        oninput={(e) => {
                          convertQuality = Number(e.currentTarget.value) || 85;
                        }}
                      />
                    </div>
                  {/if}

                  <div class="option-group">
                    <label class="field-label" for="convert-pages-input">
                      Pages (empty = all pages)
                    </label>
                    <input
                      id="convert-pages-input"
                      type="text"
                      class="text-input"
                      value={convertPagesSpec}
                      oninput={(e) =>
                        handleRangesInputEdit(e.currentTarget.value, 'convert')}
                      placeholder="All pages"
                    />
                  </div>
                {:else if convertAction === 'from-images'}
                  <div class="option-group">
                    <span class="field-label">Page Size</span>
                    <div class="segmented-control">
                      {#each ['fit', 'a4', 'letter'] as ps (ps)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={convertPageSize === ps}
                          onclick={() => {
                            convertPageSize = ps as ConvertPageSizeUi;
                          }}
                        >
                          {ps.toUpperCase()}
                        </button>
                      {/each}
                    </div>
                  </div>

                  <div class="option-group">
                    <span class="field-label">Margin</span>
                    <div class="segmented-control">
                      {#each [0, 18, 36] as m (m)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={convertMarginPt === m}
                          onclick={() => {
                            convertMarginPt = m;
                          }}
                        >
                          {m} pt
                        </button>
                      {/each}
                    </div>
                  </div>
                {:else if convertAction === 'to-markdown'}
                  <div class="option-group">
                    <label class="field-label" for="convert-md-pages-input">
                      Pages (empty = all pages)
                    </label>
                    <input
                      id="convert-md-pages-input"
                      type="text"
                      class="text-input"
                      value={convertPagesSpec}
                      oninput={(e) =>
                        handleRangesInputEdit(e.currentTarget.value, 'convert')}
                      placeholder="All pages"
                    />
                  </div>

                  <div class="option-group">
                    <span class="field-label">Output Markdown File</span>
                    <div class="path-picker-row">
                      <span class="path-preview">
                        {convertOutPath
                          ? basename(convertOutPath)
                          : primaryFile
                            ? `${basename(primaryFile.path).replace(/\.pdf$/i, '')}.md`
                            : 'output.md'}
                      </span>
                      <button
                        type="button"
                        class="small-btn"
                        onclick={async () => {
                          const chosen = await pickSaveMarkdownPath('document.md');
                          if (chosen) convertOutPath = chosen;
                        }}
                      >
                        Save As…
                      </button>
                    </div>
                  </div>
                {:else}
                  <div class="option-group">
                    <span class="field-label">Page Size</span>
                    <div class="segmented-control">
                      {#each ['a4', 'letter'] as ps (ps)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={convertPageSize === ps}
                          onclick={() => {
                            convertPageSize = ps as ConvertPageSizeUi;
                          }}
                        >
                          {ps.toUpperCase()}
                        </button>
                      {/each}
                    </div>
                  </div>

                  <div class="option-group">
                    <span class="field-label">Margin</span>
                    <div class="segmented-control">
                      {#each [36, 54, 72] as m (m)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={convertMarginPt === m}
                          onclick={() => {
                            convertMarginPt = m;
                          }}
                        >
                          {m} pt
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              {:else if activeTool === 'watermark' && primaryFile}
                <div class="option-group">
                  <span class="field-label">Overlay Type</span>
                  <div class="segmented-control" role="radiogroup" aria-label="Overlay Type">
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={overlayMode === 'watermark'}
                      onclick={() => {
                        overlayMode = 'watermark';
                      }}
                    >
                      Watermark
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={overlayMode === 'number'}
                      onclick={() => {
                        overlayMode = 'number';
                      }}
                    >
                      Page Numbers
                    </button>
                  </div>
                </div>

                {#if overlayMode === 'watermark'}
                  <div class="option-group">
                    <label class="field-label" for="watermark-text-input">
                      Watermark Text
                    </label>
                    <input
                      id="watermark-text-input"
                      type="text"
                      class="text-input"
                      value={watermarkText}
                      oninput={(e) => {
                        watermarkText = e.currentTarget.value;
                      }}
                      placeholder="CONFIDENTIAL"
                    />
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="watermark-pos-select">
                      Placement
                    </label>
                    <select
                      id="watermark-pos-select"
                      class="text-input"
                      value={watermarkPosition}
                      onchange={(e) => {
                        watermarkPosition = e.currentTarget.value as WatermarkPositionUi;
                      }}
                    >
                      <option value="diagonal">Diagonal (45°)</option>
                      <option value="center">Center (Horizontal)</option>
                      <option value="top-left">Top Left</option>
                      <option value="top-right">Top Right</option>
                      <option value="bottom-left">Bottom Left</option>
                      <option value="bottom-right">Bottom Right</option>
                    </select>
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="watermark-size-range">
                      Font Size ({watermarkFontSize} pt)
                    </label>
                    <input
                      id="watermark-size-range"
                      type="range"
                      min="14"
                      max="120"
                      step="2"
                      value={watermarkFontSize}
                      oninput={(e) => {
                        watermarkFontSize = Number(e.currentTarget.value) || 48;
                      }}
                    />
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="watermark-opacity-range">
                      Opacity ({Math.round(watermarkOpacity * 100)}%)
                    </label>
                    <input
                      id="watermark-opacity-range"
                      type="range"
                      min="0.05"
                      max="0.9"
                      step="0.05"
                      value={watermarkOpacity}
                      oninput={(e) => {
                        watermarkOpacity = Number(e.currentTarget.value) || 0.25;
                      }}
                    />
                  </div>

                  <div class="option-group">
                    <span class="field-label">Color</span>
                    <div class="segmented-control">
                      {#each [
                        { label: 'Gray', hex: '#6B7280' },
                        { label: 'Red', hex: '#DC2626' },
                        { label: 'Blue', hex: '#2563EB' },
                      ] as c (c.hex)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={watermarkColor === c.hex}
                          onclick={() => {
                            watermarkColor = c.hex;
                          }}
                        >
                          {c.label}
                        </button>
                      {/each}
                    </div>
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="watermark-pages-input">
                      Target Pages (empty = all pages)
                    </label>
                    <input
                      id="watermark-pages-input"
                      type="text"
                      class="text-input"
                      value={watermarkPagesSpec}
                      oninput={(e) =>
                        handleRangesInputEdit(e.currentTarget.value, 'watermark')}
                      placeholder="All pages"
                    />
                  </div>
                {:else}
                  <div class="option-group">
                    <label class="field-label" for="number-format-input">
                      Format ({'{n}'}, {'{total}'}, {'{page}'}, {'{pages}'})
                    </label>
                    <input
                      id="number-format-input"
                      type="text"
                      class="text-input"
                      value={numberFormat}
                      oninput={(e) => {
                        numberFormat = e.currentTarget.value;
                      }}
                      placeholder={"Page {n} of {total}"}
                    />
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="number-pos-select">
                      Placement
                    </label>
                    <select
                      id="number-pos-select"
                      class="text-input"
                      value={numberPosition}
                      onchange={(e) => {
                        numberPosition = e.currentTarget.value as NumberPositionUi;
                      }}
                    >
                      <option value="bottom-center">Bottom Center</option>
                      <option value="bottom-right">Bottom Right</option>
                      <option value="bottom-left">Bottom Left</option>
                      <option value="top-center">Top Center</option>
                      <option value="top-right">Top Right</option>
                      <option value="top-left">Top Left</option>
                    </select>
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="number-start-input">
                      Start Number ({'{n}'})
                    </label>
                    <input
                      id="number-start-input"
                      type="number"
                      min="1"
                      class="text-input"
                      value={numberStart}
                      oninput={(e) => {
                        numberStart = Math.max(1, Number(e.currentTarget.value) || 1);
                      }}
                    />
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="number-size-range">
                      Font Size ({numberFontSize} pt)
                    </label>
                    <input
                      id="number-size-range"
                      type="range"
                      min="8"
                      max="36"
                      step="1"
                      value={numberFontSize}
                      oninput={(e) => {
                        numberFontSize = Number(e.currentTarget.value) || 10;
                      }}
                    />
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="number-opacity-range">
                      Opacity ({Math.round(numberOpacity * 100)}%)
                    </label>
                    <input
                      id="number-opacity-range"
                      type="range"
                      min="0.1"
                      max="1.0"
                      step="0.05"
                      value={numberOpacity}
                      oninput={(e) => {
                        numberOpacity = Number(e.currentTarget.value) || 1.0;
                      }}
                    />
                  </div>

                  <div class="option-group">
                    <span class="field-label">Color</span>
                    <div class="segmented-control">
                      {#each [
                        { label: 'Black', hex: '#111827' },
                        { label: 'Gray', hex: '#6B7280' },
                        { label: 'Blue', hex: '#2563EB' },
                      ] as c (c.hex)}
                        <button
                          type="button"
                          class="seg-btn"
                          class:active={numberColor === c.hex}
                          onclick={() => {
                            numberColor = c.hex;
                          }}
                        >
                          {c.label}
                        </button>
                      {/each}
                    </div>
                  </div>

                  <div class="option-group">
                    <label class="field-label" for="number-pages-input">
                      Target Pages (e.g. 2- to skip cover)
                    </label>
                    <input
                      id="number-pages-input"
                      type="text"
                      class="text-input"
                      value={numberPagesSpec}
                      oninput={(e) =>
                        handleRangesInputEdit(e.currentTarget.value, 'number')}
                      placeholder="All pages"
                    />
                  </div>
                {/if}

                <div class="option-group overlay-preview-group">
                  <div class="label-row">
                    <span class="field-label">Live Preview</span>
                    <span class="preview-badge">
                      Page {selectedPages.size > 0 ? Math.min(...selectedPages) : 1}
                    </span>
                  </div>
                  <div class="overlay-preview-box">
                    {#if overlayPreviewItem}
                      <img
                        src={overlayPreviewItem.data_url}
                        alt="Overlay Live Preview"
                        class="overlay-preview-img"
                        class:dimmed={overlayPreviewLoading}
                      />
                    {:else if overlayPreviewLoading}
                      <div class="preview-status-text">Rendering preview…</div>
                    {:else if overlayPreviewError}
                      <div class="preview-error-text">{overlayPreviewError}</div>
                    {:else}
                      <div class="preview-status-text">Preview loading…</div>
                    {/if}
                  </div>
                </div>
              {:else if activeTool === 'protect' && primaryFile}
                <div class="option-group">
                  <span class="field-label">Modus</span>
                  <div class="segmented-control">
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={protectAction === 'protect'}
                      onclick={() => {
                        protectAction = 'protect';
                      }}
                    >
                      Schützen
                    </button>
                    <button
                      type="button"
                      class="seg-btn"
                      class:active={protectAction === 'unlock'}
                      onclick={() => {
                        protectAction = 'unlock';
                        if (!protectPassword && sessionPassword) {
                          protectPassword = sessionPassword;
                        }
                      }}
                    >
                      Entsperren
                    </button>
                  </div>
                </div>

                {#if protectAction === 'protect'}
                  <label class="checkbox-row">
                    <input
                      type="checkbox"
                      checked={protectEnableProtection}
                      onchange={(e) => {
                        protectEnableProtection = e.currentTarget.checked;
                      }}
                    />
                    <span>Passwortschutz aktivieren</span>
                  </label>

                  {#if protectEnableProtection}
                    <div class="option-group">
                      <label class="field-label" for="protect-pw-input">
                        Passwort
                      </label>
                      <input
                        id="protect-pw-input"
                        type="password"
                        class="text-input"
                        value={protectPassword}
                        oninput={(e) => {
                          protectPassword = e.currentTarget.value;
                        }}
                        placeholder="Passwort zum Öffnen…"
                      />
                    </div>

                    <div class="option-group">
                      <label class="field-label" for="protect-pw-confirm-input">
                        Passwort bestätigen
                      </label>
                      <input
                        id="protect-pw-confirm-input"
                        type="password"
                        class="text-input"
                        value={protectPasswordConfirm}
                        oninput={(e) => {
                          protectPasswordConfirm = e.currentTarget.value;
                        }}
                        placeholder="Passwort wiederholen…"
                      />
                      {#if protectPasswordConfirm && protectPassword !== protectPasswordConfirm}
                        <span class="field-hint error">Passwörter stimmen nicht überein</span>
                      {/if}
                    </div>

                    <div class="option-group">
                      <label class="field-label" for="protect-owner-pw-input">
                        Owner-Passwort (optional)
                      </label>
                      <input
                        id="protect-owner-pw-input"
                        type="password"
                        class="text-input"
                        value={protectOwnerPassword}
                        oninput={(e) => {
                          protectOwnerPassword = e.currentTarget.value;
                        }}
                        placeholder="Optionales Admin-Passwort…"
                      />
                    </div>

                    <div class="option-group">
                      <span class="field-label">Berechtigungen</span>
                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          checked={protectAllowPrint}
                          onchange={(e) => {
                            protectAllowPrint = e.currentTarget.checked;
                          }}
                        />
                        <span>Drucken erlauben</span>
                      </label>

                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          checked={protectAllowCopy}
                          onchange={(e) => {
                            protectAllowCopy = e.currentTarget.checked;
                          }}
                        />
                        <span>Text kopieren erlauben</span>
                      </label>

                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          checked={protectAllowModify}
                          onchange={(e) => {
                            protectAllowModify = e.currentTarget.checked;
                          }}
                        />
                        <span>Ändern erlauben</span>
                      </label>
                    </div>
                  {/if}
                {:else}
                  <div class="option-group">
                    <label class="field-label" for="protect-unlock-pw-input">
                      Passwort zum Entsperren
                    </label>
                    <input
                      id="protect-unlock-pw-input"
                      type="password"
                      class="text-input"
                      value={protectPassword}
                      oninput={(e) => {
                        protectPassword = e.currentTarget.value;
                      }}
                      placeholder="Passwort eingeben…"
                    />
                  </div>
                {/if}
              {:else if activeTool === 'info' && primaryFile}
                <InfoPanel
                  file={primaryFile}
                  {sessionPassword}
                  {isRunning}
                  onSaveSuccess={(outputPath, noticeTitle, noticeSubtitle) => {
                    toastNotice = {
                      title: noticeTitle,
                      subtitle: noticeSubtitle,
                      targetPath: outputPath,
                    };
                    void loadPaths([outputPath], false);
                  }}
                  onRequestPassword={() => {
                    passwordDialogOpen = true;
                  }}
                />
              {:else}
                <div class="option-group">
                  <span class="field-label">Selected Document</span>
                  <div class="summary-pill">{basename(loadedFiles[0].path)}</div>
                </div>
              {/if}
            </div>

            <div class="options-footer">
              {#if activeTool === 'merge' || activeTool === 'split' || activeTool === 'pages' || activeTool === 'compress' || activeTool === 'convert' || activeTool === 'watermark' || activeTool === 'protect'}
                <button
                  type="button"
                  class="primary-action-btn"
                  disabled={isRunning}
                  onclick={() => void handleRunPrimaryAction()}
                >
                  {#if isRunning}
                    Working…
                  {:else if activeTool === 'pages'}
                    {pagesAction === 'rotate'
                      ? 'Rotate Pages'
                      : pagesAction === 'delete'
                        ? 'Delete Pages'
                        : pagesAction === 'reorder'
                          ? 'Reorder Pages'
                          : 'Extract Pages'}
                  {:else if activeTool === 'convert'}
                    {convertAction === 'to-images'
                      ? `Export ${convertFormat.toUpperCase()} Images`
                      : convertAction === 'from-images'
                        ? 'Create PDF from Images'
                        : convertAction === 'to-markdown'
                          ? 'Export Markdown (.md)'
                          : 'Render PDF from Markdown'}
                  {:else if activeTool === 'protect'}
                    {protectAction === 'protect' ? 'Encrypt PDF' : 'Unlock PDF'}
                  {:else if activeTool === 'watermark'}
                    {overlayMode === 'watermark' ? 'Apply Watermark' : 'Apply Page Numbers'}
                  {:else}
                    {currentToolMeta.actionLabel}
                  {/if}
                </button>
              {/if}
            </div>
          </aside>
        </div>
      {/if}
    </main>
  </div>

  <Toast
    notice={toastNotice}
    onReveal={(path) => {
      void revealInFinder(path);
    }}
    onOpen={(path) => {
      void openPathInSystem(path);
    }}
    onDismiss={() => {
      toastNotice = null;
    }}
  />

  <StatusBar
    files={loadedFiles}
    {progress}
    {lang}
    onCancel={() => {
      void cancelActiveOperation();
    }}
  />

  <CommandPalette
    open={paletteOpen}
    {activeTool}
    {theme}
    {lang}
    onClose={() => {
      paletteOpen = false;
    }}
    onSelectTool={(id) => {
      activeTool = id;
    }}
    onOpenFiles={() => {
      void handleChooseFiles(false);
    }}
    onSelectTheme={(nextTheme) => {
      theme = nextTheme;
    }}
    onOpenSettings={() => {
      settingsDialogOpen = true;
    }}
    onSelectLanguage={(nextLang) => {
      lang = nextLang;
      saveLanguage(nextLang);
    }}
  />

  {#if passwordDialogOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="pw-modal-backdrop" onclick={handlePasswordCancel}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="pw-modal-card" onclick={(e) => e.stopPropagation()}>
        <div class="pw-modal-header">
          <div class="pw-modal-title-row">
            <Lock size={18} strokeWidth={2} class="pw-modal-icon" />
            <h2 class="pw-modal-title">{t.passwordRequired}</h2>
          </div>
          <button
            type="button"
            class="pw-modal-close"
            aria-label={t.close}
            onclick={handlePasswordCancel}
          >
            <X size={16} />
          </button>
        </div>

        <p class="pw-modal-desc">
          {t.passwordPrompt}
        </p>

        <form onsubmit={(e) => { e.preventDefault(); void handlePasswordSubmit(); }}>
          <div class="pw-modal-field">
            <!-- svelte-ignore a11y_autofocus -->
            <input
              type="password"
              class="pw-modal-input"
              placeholder={t.passwordPlaceholder}
              value={passwordPromptInput}
              oninput={(e) => {
                passwordPromptInput = e.currentTarget.value;
                passwordPromptError = null;
              }}
              autofocus
            />
          </div>

          {#if passwordPromptError}
            <div class="pw-modal-error">
              {passwordPromptError}
            </div>
          {/if}

          {#if passwordPromptAttempts >= 3}
            <div class="pw-modal-hint">
              {lang === 'de' ? 'Möglicherweise handelt es sich um ein Owner-Passwort mit abweichenden Rechten.' : 'This document might require an Owner password to grant edit permissions.'}
            </div>
          {/if}

          <div class="pw-modal-actions">
            <button
              type="button"
              class="ghost-btn"
              onclick={handlePasswordCancel}
            >
              {t.cancel}
            </button>
            <button
              type="submit"
              class="primary-action-btn modal-submit"
              disabled={!passwordPromptInput.trim()}
            >
              {t.unlockDocument}
            </button>
          </div>
        </form>
      </div>
    </div>
  {/if}

  <SettingsModal
    open={settingsDialogOpen}
    {lang}
    {theme}
    onClose={() => {
      settingsDialogOpen = false;
    }}
    onSelectLanguage={(nextLang) => {
      lang = nextLang;
      saveLanguage(nextLang);
    }}
    onSelectTheme={(nextTheme) => {
      theme = nextTheme;
    }}
  />
</div>

<style>
  .app-shell {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background-color: var(--bg);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    color: var(--text);
  }

  .workspace-row {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .main-column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background-color: transparent;
  }

  .titlebar {
    height: var(--titlebar-height);
    min-height: var(--titlebar-height);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border);
    background-color: var(--sidebar-surface);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
  }

  .titlebar:active {
    cursor: grabbing;
  }

  .titlebar-title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 13px;
  }

  .app-name {
    font-weight: 600;
    color: var(--text);
  }

  .title-sep {
    color: var(--text-muted);
  }

  .tool-title {
    color: var(--text-muted);
    font-weight: 500;
  }

  .titlebar-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .ghost-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 4px var(--space-2);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .ghost-btn:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .error-banner {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-3) var(--space-4) 0;
    padding: var(--space-2) var(--space-3);
    background-color: var(--danger-subtle);
    border: 1px solid var(--danger);
    border-radius: var(--radius-control);
    color: var(--danger);
  }

  .error-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    font-size: 12px;
  }

  .error-dismiss {
    border: none;
    background: transparent;
    color: var(--danger);
    cursor: pointer;
    padding: var(--space-1);
  }

  .loaded-workspace {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .options-panel {
    width: 280px;
    min-width: 280px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: var(--space-6);
    background-color: var(--surface);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border-left: 1px solid var(--border);
    box-shadow: inset 1px 0 0 0 rgba(255, 255, 255, 0.08);
    overflow-y: auto;
  }

  .options-body {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .panel-header {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .panel-heading {
    font-size: 15px;
    font-weight: 600;
  }

  .panel-description {
    font-size: 12px;
    color: var(--text-muted);
  }

  .option-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .field-label {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .inline-link-btn {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    border: none;
    background: transparent;
    color: var(--accent);
    font-size: 11px;
    cursor: pointer;
  }

  .segmented-control {
    display: flex;
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    padding: 2px;
    gap: 2px;
  }

  .seg-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 5px 6px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--transition-fast),
      color var(--transition-fast);
  }

  .seg-btn:hover {
    color: var(--text);
  }

  .seg-btn.active {
    background-color: var(--surface);
    color: var(--accent);
    font-weight: 600;
    border: 1px solid var(--border);
  }

  .text-input {
    width: 100%;
    padding: 7px var(--space-3);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
    font-size: 12px;
    transition: border-color var(--transition-fast);
  }

  .text-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 12px;
    color: var(--text);
    cursor: pointer;
  }

  .summary-pill {
    padding: var(--space-2) var(--space-3);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    font-size: 12px;
    font-weight: 500;
  }

  .path-picker-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: 6px var(--space-2) 6px var(--space-3);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
  }

  .path-preview {
    font-size: 12px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .small-btn {
    padding: 3px var(--space-2);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .small-btn:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .options-footer {
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }

  .primary-action-btn {
    width: 100%;
    padding: var(--space-2) var(--space-4);
    background-color: var(--accent);
    color: #ffffff;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    font-weight: 600;
    cursor: pointer;
    transition: opacity var(--transition-fast);
  }

  .primary-action-btn:hover:not(:disabled) {
    opacity: 0.92;
  }

  .primary-action-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .convert-source-pane {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-6);
  }

  .source-card {
    width: 100%;
    max-width: 420px;
    padding: var(--space-5);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .source-card-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .source-path-list {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    max-height: 240px;
    overflow-y: auto;
  }

  .source-path-item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-2);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    font-size: 12px;
  }

  .source-index {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .source-name {
    margin: 0;
    font-size: 12px;
    word-break: break-all;
  }

  .overlay-preview-group {
    margin-top: var(--space-2);
  }

  .preview-badge {
    font-size: 10px;
    font-weight: 600;
    padding: 1px 6px;
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--accent);
  }

  .overlay-preview-box {
    width: 100%;
    min-height: 140px;
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-2);
    overflow: hidden;
  }

  .overlay-preview-img {
    max-width: 100%;
    max-height: 220px;
    object-fit: contain;
    border-radius: 4px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
    transition: opacity var(--transition-fast);
  }

  .overlay-preview-img.dimmed {
    opacity: 0.5;
  }

  .preview-status-text {
    font-size: 11px;
    color: var(--text-muted);
  }

  .preview-error-text {
    font-size: 11px;
    color: var(--danger);
    text-align: center;
    padding: var(--space-2);
  }

  .lock-status-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 500;
    padding: 2px 7px;
    border-radius: 9999px;
  }

  .lock-status-badge.unencrypted {
    color: var(--text-muted);
    background-color: var(--border);
  }

  .lock-status-badge.unlocked {
    color: #10b981;
    background-color: rgba(16, 185, 129, 0.12);
  }

  .lock-status-badge.readonly,
  .lock-status-badge.locked {
    color: #ef4444;
    background-color: rgba(239, 68, 68, 0.12);
  }

  .field-hint.error {
    color: #ef4444;
    font-size: 11px;
    margin-top: 4px;
    display: block;
  }

  .pw-modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(0, 0, 0, 0.35);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .pw-modal-card {
    width: 400px;
    max-width: 90vw;
    background-color: var(--surface);
    backdrop-filter: blur(36px) saturate(190%);
    -webkit-backdrop-filter: blur(36px) saturate(190%);
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-card);
    padding: var(--space-4);
    box-shadow: var(--glass-glow), var(--shadow-floating);
  }

  .pw-modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-2);
  }

  .pw-modal-title-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .pw-modal-title {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
  }

  .pw-modal-close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
  }

  .pw-modal-close:hover {
    color: var(--text);
    background-color: var(--border);
  }

  .pw-modal-desc {
    font-size: 13px;
    color: var(--text-muted);
    margin: 0 0 var(--space-3) 0;
  }

  .pw-modal-field {
    margin-bottom: var(--space-3);
  }

  .pw-modal-input {
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
    font-size: 13px;
  }

  .pw-modal-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .pw-modal-error {
    color: #ef4444;
    font-size: 12px;
    margin-bottom: var(--space-2);
  }

  .pw-modal-hint {
    color: var(--text-muted);
    font-size: 11px;
    margin-bottom: var(--space-3);
    line-height: 1.4;
  }

  .pw-modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }

  .modal-submit {
    padding: var(--space-2) var(--space-4);
  }
</style>
