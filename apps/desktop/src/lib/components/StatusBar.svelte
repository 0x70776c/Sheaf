<script lang="ts">
  import { basename, formatBytes } from '../tools';
  import type { FileInfo, ProgressEventPayload } from '../types';

  interface Props {
    files: FileInfo[];
    progress: ProgressEventPayload | null;
    onCancel: () => void;
  }

  let { files, progress, onCancel }: Props = $props();

  let statusText = $derived.by(() => {
    if (files.length === 0) {
      return 'No file selected · 100% Offline';
    }
    if (files.length === 1) {
      const f = files[0];
      const pagesLabel =
        f.pages === null
          ? 'Encrypted'
          : `${f.pages} ${f.pages === 1 ? 'page' : 'pages'}`;
      return `${basename(f.path)} · ${pagesLabel} · ${formatBytes(f.size_bytes)}`;
    }
    const totalPages = files.reduce((acc, f) => acc + (f.pages ?? 0), 0);
    const totalBytes = files.reduce((acc, f) => acc + f.size_bytes, 0);
    return `${files.length} files · ${totalPages} pages · ${formatBytes(totalBytes)}`;
  });

  let progressPercent = $derived.by(() => {
    if (!progress || progress.total === 0) return 0;
    return Math.min(100, Math.round((progress.done / progress.total) * 100));
  });
</script>

<footer class="statusbar" role="status" aria-live="polite">
  {#if progress}
    <div class="progress-track" aria-hidden="true">
      <div class="progress-fill" style={`width: ${progressPercent}%`}></div>
    </div>
  {/if}

  <div class="statusbar-left">
    <span class="status-summary">{statusText}</span>
  </div>

  <div class="statusbar-right">
    {#if progress}
      <span class="progress-text">
        {progress.op} ({progress.done}/{progress.total})
      </span>
      <button type="button" class="cancel-btn" onclick={onCancel}>
        Cancel
      </button>
    {:else}
      <span class="shortcut-hint">⌘K Command Palette · ⌘O Open</span>
    {/if}
  </div>
</footer>

<style>
  .statusbar {
    position: relative;
    height: var(--statusbar-height);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--space-4);
    background-color: var(--surface);
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
  }

  .progress-track {
    position: absolute;
    top: -2px;
    left: 0;
    right: 0;
    height: 2px;
    background-color: var(--border);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background-color: var(--accent);
    transition: width var(--transition-fast);
  }

  .statusbar-left,
  .statusbar-right {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .status-summary {
    color: var(--text-muted);
  }

  .shortcut-hint {
    font-size: 11px;
    color: var(--text-muted);
  }

  .cancel-btn {
    padding: 1px var(--space-2);
    border: 1px solid var(--danger);
    border-radius: 4px;
    background-color: var(--danger-subtle);
    color: var(--danger);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
  }
</style>
