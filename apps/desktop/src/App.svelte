<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { FolderOpen, X, AlertCircle } from '@lucide/svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import DropZone from './lib/components/DropZone.svelte';
  import CommandPalette from './lib/components/CommandPalette.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import { TOOLS, basename, formatBytes, getToolById } from './lib/tools';
  import {
    cancelActiveOperation,
    extractErrorMessage,
    fetchThumbnails,
    isTauriRuntime,
    onProgressEvent,
    openPdf,
    pickPdfFiles,
  } from './lib/tauri';
  import type {
    CommandErrorPayload,
    FileInfo,
    ProgressEventPayload,
    ThemePreference,
    ThumbnailItem,
    ToolId,
  } from './lib/types';

  let activeTool = $state<ToolId>('merge');
  let theme = $state<ThemePreference>('system');
  let paletteOpen = $state(false);
  let isDraggingOver = $state(false);
  let loadedFiles = $state<FileInfo[]>([]);
  let thumbnails = $state<ThumbnailItem[]>([]);
  let loadingThumbnails = $state(false);
  let progress = $state<ProgressEventPayload | null>(null);
  let errorBanner = $state<CommandErrorPayload | null>(null);

  let currentToolMeta = $derived(getToolById(activeTool));

  $effect(() => {
    const root = document.documentElement;
    if (theme === 'system') {
      root.removeAttribute('data-theme');
    } else {
      root.setAttribute('data-theme', theme);
    }
  });

  async function loadPaths(paths: string[]) {
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
      loadedFiles = report.files;

      const primary = report.files[0];
      if (primary && !primary.encrypted && (primary.pages ?? 0) > 0) {
        loadingThumbnails = true;
        thumbnails = [];
        const previewCount = Math.min(primary.pages ?? 1, 20);
        const pageNumbers = Array.from({ length: previewCount }, (_, i) => i + 1);
        thumbnails = await fetchThumbnails(primary.path, pageNumbers, 160);
      } else {
        thumbnails = [];
      }
    } catch (err) {
      errorBanner = extractErrorMessage(err);
    } finally {
      loadingThumbnails = false;
    }
  }

  async function handleChooseFiles() {
    const allowMultiple = activeTool === 'merge';
    const selected = await pickPdfFiles(allowMultiple);
    if (selected.length > 0) {
      await loadPaths(selected);
    }
  }

  function handleClearFiles() {
    loadedFiles = [];
    thumbnails = [];
    errorBanner = null;
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    const isCmd = event.metaKey || event.ctrlKey;
    if (isCmd && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      paletteOpen = !paletteOpen;
      return;
    }
    if (isCmd && event.key.toLowerCase() === 'o') {
      event.preventDefault();
      paletteOpen = false;
      void handleChooseFiles();
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
    if (event.key === 'Escape' && !paletteOpen && errorBanner) {
      errorBanner = null;
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
              void loadPaths(event.payload.paths);
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
              onclick={handleChooseFiles}
              aria-label="Open another PDF file (Command+O)"
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
          onChooseFiles={handleChooseFiles}
          onDropFiles={(paths) => {
            void loadPaths(paths);
          }}
          onDragStateChange={(hovering) => {
            isDraggingOver = hovering;
          }}
        />
      {:else}
        <div class="loaded-workspace">
          <section class="preview-pane" aria-label="Page Thumbnails">
            {#if loadingThumbnails}
              <div class="thumbnail-grid">
                {#each Array.from({ length: 8 }, (_, idx) => idx + 1) as pagePlaceholder (pagePlaceholder)}
                  <div class="thumb-card skeleton" aria-hidden="true">
                    <div class="thumb-skeleton-box"></div>
                    <span class="thumb-label">Page {pagePlaceholder}</span>
                  </div>
                {/each}
              </div>
            {:else if thumbnails.length > 0}
              <div class="thumbnail-grid">
                {#each thumbnails as item (item.page)}
                  <div class="thumb-card">
                    <img
                      src={item.data_url}
                      alt={`Page ${item.page}`}
                      width={item.width_px}
                      height={item.height_px}
                      loading="lazy"
                    />
                    <span class="thumb-label">{item.page}</span>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="file-summary-list">
                {#each loadedFiles as file (file.path)}
                  <div class="file-card">
                    <div class="file-card-name">{basename(file.path)}</div>
                    <div class="file-card-meta">
                      {file.encrypted
                        ? 'Password-protected'
                        : `${file.pages ?? 0} pages`} · {formatBytes(file.size_bytes)}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </section>

          <aside class="options-panel" aria-label={`${currentToolMeta.label} Options`}>
            <div class="options-body">
              <h2 class="panel-heading">{currentToolMeta.label}</h2>
              <p class="panel-description">{currentToolMeta.description}</p>

              <div class="selected-files-summary">
                <span class="field-label">Selected Document</span>
                <div class="field-value">{basename(loadedFiles[0].path)}</div>
              </div>
            </div>

            <div class="options-footer">
              <button type="button" class="primary-action-btn">
                {currentToolMeta.actionLabel}
              </button>
            </div>
          </aside>
        </div>
      {/if}
    </main>
  </div>

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
      void handleChooseFiles();
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

  .preview-pane {
    flex: 1;
    min-width: 0;
    padding: var(--space-6);
    overflow-y: auto;
  }

  .thumbnail-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: var(--space-4);
  }

  .thumb-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }

  .thumb-card img {
    width: 100%;
    height: auto;
    border-radius: 4px;
    border: 1px solid var(--border);
    display: block;
  }

  .thumb-skeleton-box {
    width: 100%;
    aspect-ratio: 1 / 1.414;
    background-color: var(--border);
    border-radius: 4px;
  }

  .thumb-label {
    font-size: 11px;
    color: var(--text-muted);
  }

  .file-summary-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .file-card {
    padding: var(--space-3) var(--space-4);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .file-card-name {
    font-weight: 500;
  }

  .file-card-meta {
    font-size: 12px;
    color: var(--text-muted);
  }

  .options-panel {
    width: 260px;
    min-width: 260px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: var(--space-6);
    background-color: var(--surface);
    border-left: 1px solid var(--border);
  }

  .options-body {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .panel-heading {
    font-size: 15px;
    font-weight: 600;
  }

  .panel-description {
    font-size: 12px;
    color: var(--text-muted);
  }

  .selected-files-summary {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
  }

  .field-label {
    font-size: 11px;
    color: var(--text-muted);
  }

  .field-value {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

  .primary-action-btn:hover {
    opacity: 0.92;
  }
</style>
