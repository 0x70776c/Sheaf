<script lang="ts">
  import { FolderOpen } from '@lucide/svelte';
  import type { ToolMetadata } from '../tools';

  interface Props {
    tool: ToolMetadata;
    isDraggingOver: boolean;
    onChooseFiles: () => void;
    onDropFiles: (paths: string[]) => void;
    onDragStateChange: (hovering: boolean) => void;
  }

  let {
    tool,
    isDraggingOver,
    onChooseFiles,
    onDropFiles,
    onDragStateChange,
  }: Props = $props();

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    if (!isDraggingOver) {
      onDragStateChange(true);
    }
  }

  function handleDragLeave(event: DragEvent) {
    event.preventDefault();
    onDragStateChange(false);
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault();
    onDragStateChange(false);
    const files = event.dataTransfer?.files;
    if (!files || files.length === 0) return;
    const paths: string[] = [];
    for (let i = 0; i < files.length; i++) {
      const f = files[i] as File & { path?: string };
      if (f.path) {
        paths.push(f.path);
      } else if (f.name) {
        paths.push(f.name);
      }
    }
    if (paths.length > 0) {
      onDropFiles(paths);
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onChooseFiles();
    }
  }
</script>

<div class="dropzone-wrapper">
  <div
    class="dropzone-card"
    class:drag-over={isDraggingOver}
    role="button"
    tabindex="0"
    aria-label={`${tool.emptyPrompt}. Press Enter or Command+O to choose a PDF file.`}
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    onclick={onChooseFiles}
    onkeydown={handleKeydown}
  >
    <p class="dropzone-sentence">{tool.emptyPrompt}</p>
    <button
      type="button"
      class="choose-btn"
      onclick={(e) => {
        e.stopPropagation();
        onChooseFiles();
      }}
    >
      <FolderOpen size={15} strokeWidth={1.5} aria-hidden="true" />
      <span>Choose PDF File…</span>
      <kbd class="shortcut-hint">⌘O</kbd>
    </button>
  </div>
</div>

<style>
  .dropzone-wrapper {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8);
  }

  .dropzone-card {
    width: 100%;
    max-width: 440px;
    min-height: 220px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    padding: var(--space-8);
    background-color: var(--surface);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border: 1px dashed var(--border);
    border-radius: var(--radius-card);
    text-align: center;
    cursor: pointer;
    box-shadow: var(--shadow-glass);
    transition:
      border-color var(--transition-fast),
      background-color var(--transition-fast),
      transform var(--transition-fast);
  }

  .dropzone-card:hover {
    border-color: var(--accent);
    background-color: var(--surface-hover);
  }

  .dropzone-card.drag-over {
    border-style: solid;
    border-color: var(--accent);
    background-color: var(--accent-subtle);
    transform: scale(1.015);
  }

  .dropzone-sentence {
    font-size: 14px;
    font-weight: 500;
    color: var(--text);
  }

  .choose-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    background-color: var(--surface-hover);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
    font-weight: 500;
    cursor: pointer;
    box-shadow: var(--glass-glow), 0 1px 3px rgba(0, 0, 0, 0.04);
    transition:
      border-color var(--transition-fast),
      background-color var(--transition-fast);
  }

  .choose-btn:hover {
    border-color: var(--accent);
    background-color: var(--surface);
  }

  .shortcut-hint {
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--text-muted);
    margin-left: var(--space-1);
  }
</style>
