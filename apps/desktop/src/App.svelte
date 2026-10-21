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
  } from '@lucide/svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import DropZone from './lib/components/DropZone.svelte';
  import CommandPalette from './lib/components/CommandPalette.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import ThumbnailGrid from './lib/components/ThumbnailGrid.svelte';
  import MergeFileList from './lib/components/MergeFileList.svelte';
  import Toast from './lib/components/Toast.svelte';
  import { TOOLS, basename, formatBytes, getToolById } from './lib/tools';
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
    pickOutputDirectory,
    pickPdfFiles,
    pickSavePdfPath,
    revealInFinder,
    runCompressCommand,
    runMergeCommand,
    runPagesCommand,
    runSplitCommand,
  } from './lib/tauri';
  import type {
    CommandErrorPayload,
    CompressPresetUi,
    FileInfo,
    PagesActionUi,
    ProgressEventPayload,
    SplitModeUi,
    ThemePreference,
    ThumbnailItem,
    ToastNotice,
    ToolId,
  } from './lib/types';

  let activeTool = $state<ToolId>('merge');
  let theme = $state<ThemePreference>('system');
  let paletteOpen = $state(false);
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

  // Execution & feedback state
  let isRunning = $state(false);
  let progress = $state<ProgressEventPayload | null>(null);
  let errorBanner = $state<CommandErrorPayload | null>(null);
  let toastNotice = $state<ToastNotice | null>(null);

  let currentToolMeta = $derived(getToolById(activeTool));
  let primaryFile = $derived<FileInfo | undefined>(loadedFiles[0]);
  let primaryTotalPages = $derived(primaryFile?.pages ?? 0);

  $effect(() => {
    const root = document.documentElement;
    if (theme === 'system') {
      root.removeAttribute('data-theme');
    } else {
      root.setAttribute('data-theme', theme);
    }
  });

  async function loadThumbnailsUpTo(file: FileInfo, maxPage: number) {
    if (file.encrypted || !file.pages || file.pages <= 0) return;
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
      const items = await fetchThumbnails(file.path, missing, 160);
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
    const pdfPaths = paths.filter((p) => p.toLowerCase().endsWith('.pdf'));
    if (pdfPaths.length === 0) {
      errorBanner = {
        code: 'invalid_selection',
        message: 'Please select at least one .pdf file.',
        hint: null,
      };
      return;
    }

    errorBanner = null;
    if (!isTauriRuntime()) {
      return;
    }

    try {
      const report = await openPdf(pdfPaths);
      if (append && activeTool === 'merge') {
        loadedFiles = [...loadedFiles, ...report.files];
      } else {
        loadedFiles = report.files;
        thumbnailsMap = new Map();
        selectedPages = new Set();
        const first = report.files[0];
        if (first) {
          const total = first.pages ?? 1;
          splitRangesInput = total > 1 ? `1-${Math.min(3, total)}` : '1';
          pagesSpecInput = '';
          await loadThumbnailsUpTo(first, 20);
        }
      }
    } catch (err) {
      errorBanner = extractErrorMessage(err);
    }
  }

  async function handleChooseFiles(append = false) {
    const allowMultiple = activeTool === 'merge';
    const selected = await pickPdfFiles(allowMultiple);
    if (selected.length > 0) {
      await loadPaths(selected, append);
    }
  }

  function handleClearFiles() {
    loadedFiles = [];
    thumbnailsMap = new Map();
    selectedPages = new Set();
    errorBanner = null;
  }

  function handleGridSelectionChange(nextSelected: Set<number>) {
    selectedPages = nextSelected;
    const formatted = formatSelectedPagesToRanges(nextSelected);
    if (activeTool === 'split' && splitMode === 'ranges' && formatted) {
      splitRangesInput = formatted;
    } else if (activeTool === 'pages' && pagesAction !== 'reorder') {
      pagesSpecInput = formatted;
    }
  }

  function handleRangesInputEdit(value: string, target: 'split' | 'pages') {
    if (target === 'split') {
      splitRangesInput = value;
    } else {
      pagesSpecInput = value;
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
    if (isRunning || loadedFiles.length === 0) return;
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
      }
    } catch (err) {
      errorBanner = extractErrorMessage(err);
    } finally {
      isRunning = false;
      progress = null;
    }
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    const isCmd = event.metaKey || event.ctrlKey;
    const targetTag = (event.target as HTMLElement | null)?.tagName;
    const isInputFocused =
      targetTag === 'INPUT' || targetTag === 'TEXTAREA' || targetTag === 'SELECT';

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
      onSelectTool={(id) => {
        activeTool = id;
      }}
      onOpenCommandPalette={() => {
        paletteOpen = true;
      }}
    />

    <main class="main-column" aria-label={`${currentToolMeta.label} Workspace`}>
      <header class="titlebar" data-tauri-drag-region>
        <div class="titlebar-title" data-tauri-drag-region>
          <span class="app-name">Sheaf</span>
          <span class="title-sep" aria-hidden="true">·</span>
          <span class="tool-title">{currentToolMeta.label}</span>
        </div>

        <div class="titlebar-actions">
          {#if loadedFiles.length > 0}
            <button
              type="button"
              class="ghost-btn"
              onclick={() => void handleChooseFiles(false)}
              aria-label="Open PDF file (Command+O)"
            >
              <FolderOpen size={14} strokeWidth={1.5} aria-hidden="true" />
              <span>Open…</span>
            </button>
            <button
              type="button"
              class="ghost-btn"
              onclick={handleClearFiles}
              aria-label="Clear loaded files"
            >
              <X size={14} strokeWidth={1.5} aria-hidden="true" />
              <span>Clear</span>
            </button>
          {/if}
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

      {#if loadedFiles.length === 0}
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
              {:else if activeTool === 'info' && primaryFile}
                <div class="info-list">
                  <div class="info-item">
                    <span class="field-label">File</span>
                    <span class="info-val">{basename(primaryFile.path)}</span>
                  </div>
                  <div class="info-item">
                    <span class="field-label">Size</span>
                    <span class="info-val">{formatBytes(primaryFile.size_bytes)}</span>
                  </div>
                  <div class="info-item">
                    <span class="field-label">PDF Version</span>
                    <span class="info-val">{primaryFile.pdf_version ?? 'Unknown'}</span>
                  </div>
                  <div class="info-item">
                    <span class="field-label">Pages</span>
                    <span class="info-val">{primaryFile.pages ?? 'Encrypted'}</span>
                  </div>
                  <div class="info-item">
                    <span class="field-label">Outlines</span>
                    <span class="info-val">
                      {primaryFile.has_outlines
                        ? `${primaryFile.outline_count} bookmarks`
                        : 'None'}
                    </span>
                  </div>
                  {#if primaryFile.metadata.title}
                    <div class="info-item">
                      <span class="field-label">Title</span>
                      <span class="info-val">{primaryFile.metadata.title}</span>
                    </div>
                  {/if}
                  {#if primaryFile.metadata.author}
                    <div class="info-item">
                      <span class="field-label">Author</span>
                      <span class="info-val">{primaryFile.metadata.author}</span>
                    </div>
                  {/if}
                </div>
              {:else}
                <div class="option-group">
                  <span class="field-label">Selected Document</span>
                  <div class="summary-pill">{basename(loadedFiles[0].path)}</div>
                </div>
              {/if}
            </div>

            <div class="options-footer">
              {#if activeTool === 'merge' || activeTool === 'split' || activeTool === 'pages' || activeTool === 'compress'}
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
    onCancel={() => {
      void cancelActiveOperation();
    }}
  />

  <CommandPalette
    open={paletteOpen}
    {activeTool}
    {theme}
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
  />
</div>

<style>
  .app-shell {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background-color: var(--bg);
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
    background-color: var(--bg);
  }

  .titlebar {
    height: var(--titlebar-height);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border);
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
    border-left: 1px solid var(--border);
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

  .info-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-2) var(--space-3);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
  }

  .info-val {
    font-size: 12px;
    font-weight: 500;
    word-break: break-all;
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
</style>
