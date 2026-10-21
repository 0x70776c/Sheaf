<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { TOOLS } from '../tools';
  import type { ToolId } from '../types';
  import ToolIcon from './ToolIcon.svelte';

  interface Props {
    activeTool: ToolId;
    onSelectTool: (tool: ToolId) => void;
    onOpenCommandPalette: () => void;
  }

  let { activeTool, onSelectTool, onOpenCommandPalette }: Props = $props();

  const primaryTools = TOOLS.filter((t) => t.group === 'primary');
  const secondaryTools = TOOLS.filter((t) => t.group === 'secondary');

  function handleNavKeydown(event: KeyboardEvent) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const index = TOOLS.findIndex((t) => t.id === activeTool);
    if (index === -1) return;
    const nextIndex =
      event.key === 'ArrowDown'
        ? (index + 1) % TOOLS.length
        : (index - 1 + TOOLS.length) % TOOLS.length;
    onSelectTool(TOOLS[nextIndex].id);
  }
</script>

<aside class="sidebar" aria-label="Tools Navigation">
  <div class="titlebar-spacer" data-tauri-drag-region></div>

  <div class="palette-trigger-wrap">
    <button
      type="button"
      class="palette-trigger"
      onclick={onOpenCommandPalette}
      aria-label="Open Command Palette (Command+K)"
    >
      <span class="palette-trigger-left">
        <Search size={14} strokeWidth={1.5} aria-hidden="true" />
        <span>Switch tool...</span>
      </span>
      <kbd class="kbd">⌘K</kbd>
    </button>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <nav class="nav-list" aria-label="PDF Tools" onkeydown={handleNavKeydown}>
    <div class="nav-group" role="group" aria-label="Core Tools">
      {#each primaryTools as item (item.id)}
        <button
          type="button"
          class="nav-item"
          class:active={activeTool === item.id}
          aria-current={activeTool === item.id ? 'page' : undefined}
          aria-label={`${item.label} (${item.shortcut})`}
          onclick={() => onSelectTool(item.id)}
        >
          <span class="active-pill" aria-hidden="true"></span>
          <ToolIcon tool={item.id} size={16} />
          <span class="nav-label">{item.label}</span>
        </button>
      {/each}
    </div>

    <div class="nav-divider" role="separator"></div>

    <div class="nav-group" role="group" aria-label="Document & Security">
      {#each secondaryTools as item (item.id)}
        <button
          type="button"
          class="nav-item"
          class:active={activeTool === item.id}
          aria-current={activeTool === item.id ? 'page' : undefined}
          aria-label={`${item.label} (${item.shortcut})`}
          onclick={() => onSelectTool(item.id)}
        >
          <span class="active-pill" aria-hidden="true"></span>
          <ToolIcon tool={item.id} size={16} />
          <span class="nav-label">{item.label}</span>
        </button>
      {/each}
    </div>
  </nav>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-width);
    min-width: var(--sidebar-width);
    height: 100%;
    display: flex;
    flex-direction: column;
    background-color: var(--sidebar-surface);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border-right: 1px solid var(--border);
  }

  .titlebar-spacer {
    height: var(--titlebar-height);
    flex-shrink: 0;
  }

  .palette-trigger-wrap {
    padding: var(--space-2) var(--space-3);
  }

  .palette-trigger {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px var(--space-2);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    cursor: pointer;
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .palette-trigger:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .palette-trigger-left {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 12px;
  }

  .kbd {
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--text-muted);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
  }

  .nav-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: var(--space-1) var(--space-3);
    overflow-y: auto;
  }

  .nav-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-divider {
    height: 1px;
    background-color: var(--border);
    margin: var(--space-3) var(--space-1);
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border: none;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--text-muted);
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--transition-fast),
      color var(--transition-fast);
  }

  .nav-item:hover {
    background-color: var(--surface);
    color: var(--text);
  }

  .nav-item.active {
    background-color: var(--accent-subtle);
    color: var(--accent);
    font-weight: 600;
  }

  .active-pill {
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 3px;
    border-radius: 999px;
    background-color: transparent;
    transition: background-color var(--transition-fast);
  }

  .nav-item.active .active-pill {
    background-color: var(--accent);
  }

  .nav-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
