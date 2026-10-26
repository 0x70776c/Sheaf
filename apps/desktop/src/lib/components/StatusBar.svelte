<script lang="ts">
  import { basename, formatBytes } from '../tools';
  import type { FileInfo, ProgressEventPayload } from '../types';
  import type { Language } from '../i18n';
  import { translations } from '../i18n';

  interface Props {
    files: FileInfo[];
    progress: ProgressEventPayload | null;
    lang?: Language;
    onCancel: () => void;
  }

  let { files, progress, lang = 'en', onCancel }: Props = $props();

  let t = $derived(translations[lang]);

  let statusText = $derived.by(() => {
    if (files.length === 0) {
      return lang === 'de'
        ? 'Keine Datei ausgewählt'
        : 'No file selected';
    }
    if (files.length === 1) {
      const f = files[0];
      const pageWord = f.pages === 1 ? (lang === 'de' ? 'Seite' : 'page') : (lang === 'de' ? 'Seiten' : 'pages');
      const pagesLabel =
        f.pages === null
          ? (lang === 'de' ? 'Geschützt' : 'Encrypted')
          : `${f.pages} ${pageWord}`;
      return `${basename(f.path)} · ${pagesLabel} · ${formatBytes(f.size_bytes)}`;
    }
    const totalPages = files.reduce((acc, f) => acc + (f.pages ?? 0), 0);
    const totalBytes = files.reduce((acc, f) => acc + f.size_bytes, 0);
    const filesWord = files.length === 1 ? (lang === 'de' ? 'Datei' : 'file') : (lang === 'de' ? 'Dateien' : 'files');
    const pagesWord = totalPages === 1 ? (lang === 'de' ? 'Seite' : 'page') : (lang === 'de' ? 'Seiten' : 'pages');
    return `${files.length} ${filesWord} · ${totalPages} ${pagesWord} · ${formatBytes(totalBytes)}`;
  });

  let progressPercent = $derived.by(() => {
    if (!progress || progress.total === 0) return 0;
    return Math.min(100, Math.round((progress.done / progress.total) * 100));
  });
</script>

<footer class="statusbar no-drag" role="status" aria-live="polite">
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
        {t.cancel}
      </button>
    {:else}
      <span class="shortcut-hint">{lang === 'de' ? '⌘K Werkzeugsuche · ⌘O Öffnen · ⌘, Einstellungen' : '⌘K Command Palette · ⌘O Open · ⌘, Settings'}</span>
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
    background-color: var(--sidebar-surface);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .shortcut-hint {
    font-size: 11px;
    color: var(--text-muted);
    opacity: 0.8;
  }

  .progress-text {
    font-size: 11px;
    font-weight: 500;
    color: var(--accent);
  }

  .cancel-btn {
    border: 1px solid var(--border);
    background-color: var(--surface);
    color: var(--text-muted);
    font-size: 11px;
    padding: 2px 8px;
    border-radius: var(--radius-control);
    cursor: pointer;
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .cancel-btn:hover {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
