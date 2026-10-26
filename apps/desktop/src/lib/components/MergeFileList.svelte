<script lang="ts">
  import { ArrowUp, ArrowDown, Plus, Trash2, FileText } from '@lucide/svelte';
  import { basename, formatBytes } from '../tools';
  import type { FileInfo } from '../types';

  interface Props {
    files: FileInfo[];
    onMoveFile: (index: number, direction: -1 | 1) => void;
    onRemoveFile: (index: number) => void;
    onAddMoreFiles: () => void;
  }

  let { files, onMoveFile, onRemoveFile, onAddMoreFiles }: Props = $props();
</script>

<div class="merge-list-pane">
  <div class="merge-toolbar">
    <span class="merge-count">
      <strong>{files.length}</strong> {files.length === 1 ? 'file' : 'files'} in merge queue
    </span>

    <button type="button" class="add-btn" onclick={onAddMoreFiles}>
      <Plus size={14} strokeWidth={1.5} aria-hidden="true" />
      <span>Add PDFs…</span>
    </button>
  </div>

  <div class="merge-list" role="list" aria-label="Files to merge">
    {#each files as file, idx (file.path + ':' + idx)}
      <div class="merge-row" role="listitem">
        <span class="row-index">{idx + 1}</span>
        <span class="row-icon">
          <FileText size={16} strokeWidth={1.5} aria-hidden="true" />
        </span>

        <div class="row-info">
          <div class="row-name">{basename(file.path)}</div>
          <div class="row-meta">
            {file.encrypted
              ? 'Password-protected'
              : `${file.pages ?? 0} ${file.pages === 1 ? 'page' : 'pages'}`} · {formatBytes(
              file.size_bytes,
            )}
          </div>
        </div>

        <div class="row-actions">
          <button
            type="button"
            class="icon-btn"
            disabled={idx === 0}
            aria-label={`Move ${basename(file.path)} up`}
            onclick={() => onMoveFile(idx, -1)}
          >
            <ArrowUp size={14} strokeWidth={1.5} aria-hidden="true" />
          </button>
          <button
            type="button"
            class="icon-btn"
            disabled={idx === files.length - 1}
            aria-label={`Move ${basename(file.path)} down`}
            onclick={() => onMoveFile(idx, 1)}
          >
            <ArrowDown size={14} strokeWidth={1.5} aria-hidden="true" />
          </button>
          <button
            type="button"
            class="icon-btn danger"
            aria-label={`Remove ${basename(file.path)}`}
            onclick={() => onRemoveFile(idx)}
          >
            <Trash2 size={14} strokeWidth={1.5} aria-hidden="true" />
          </button>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .merge-list-pane {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .merge-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-6);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-muted);
  }

  .merge-count strong {
    color: var(--text);
    font-weight: 600;
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 4px var(--space-3);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: border-color var(--transition-fast);
  }

  .add-btn:hover {
    border-color: var(--accent);
  }

  .merge-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .merge-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background-color: var(--surface);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-glass);
  }

  .row-index {
    width: 20px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-align: center;
  }

  .row-icon {
    color: var(--accent);
    display: inline-flex;
  }

  .row-info {
    flex: 1;
    min-width: 0;
  }

  .row-name {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row-meta {
    font-size: 11px;
    color: var(--text-muted);
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .icon-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text);
  }

  .icon-btn.danger:hover:not(:disabled) {
    border-color: var(--danger);
    color: var(--danger);
  }

  .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
