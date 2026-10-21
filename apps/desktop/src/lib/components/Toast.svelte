<script lang="ts">
  import { CheckCircle2, ExternalLink, Folder, X } from '@lucide/svelte';
  import type { ToastNotice } from '../types';

  interface Props {
    notice: ToastNotice | null;
    onReveal: (path: string) => void;
    onOpen: (path: string) => void;
    onDismiss: () => void;
  }

  let { notice, onReveal, onOpen, onDismiss }: Props = $props();
</script>

{#if notice}
  <div class="toast-card" role="status" aria-live="polite">
    <span class="toast-icon">
      <CheckCircle2 size={18} strokeWidth={1.5} aria-hidden="true" />
    </span>

    <div class="toast-text">
      <div class="toast-title">{notice.title}</div>
      <div class="toast-subtitle">{notice.subtitle}</div>
    </div>

    <div class="toast-actions">
      <button
        type="button"
        class="toast-btn"
        onclick={() => onReveal(notice.targetPath)}
      >
        <Folder size={13} strokeWidth={1.5} aria-hidden="true" />
        <span>Show in Finder</span>
      </button>

      <button
        type="button"
        class="toast-btn primary"
        onclick={() => onOpen(notice.targetPath)}
      >
        <ExternalLink size={13} strokeWidth={1.5} aria-hidden="true" />
        <span>Open</span>
      </button>

      <button
        type="button"
        class="dismiss-btn"
        aria-label="Dismiss notification"
        onclick={onDismiss}
      >
        <X size={14} strokeWidth={1.5} aria-hidden="true" />
      </button>
    </div>
  </div>
{/if}

<style>
  .toast-card {
    position: fixed;
    left: calc(var(--sidebar-width) + var(--space-6));
    bottom: calc(var(--statusbar-height) + var(--space-4));
    z-index: 80;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-floating);
    max-width: 560px;
  }

  .toast-icon {
    color: var(--accent);
    display: inline-flex;
    flex-shrink: 0;
  }

  .toast-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    margin-right: var(--space-2);
  }

  .toast-title {
    font-weight: 600;
    font-size: 13px;
    color: var(--text);
  }

  .toast-subtitle {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 240px;
  }

  .toast-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .toast-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 5px var(--space-2);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: border-color var(--transition-fast);
  }

  .toast-btn:hover {
    border-color: var(--accent);
  }

  .toast-btn.primary {
    background-color: var(--accent-subtle);
    border-color: var(--accent);
    color: var(--accent);
  }

  .dismiss-btn {
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    padding: var(--space-1);
    display: inline-flex;
  }

  .dismiss-btn:hover {
    color: var(--text);
  }
</style>
