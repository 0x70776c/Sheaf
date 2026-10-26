<script lang="ts">
  import { Search, Settings } from '@lucide/svelte';
  import { getLocalizedTools } from '../tools';
  import type { ToolId } from '../types';
  import type { Language } from '../i18n';
  import { translations } from '../i18n';
  import { startWindowDrag } from '../tauri';
  import ToolIcon from './ToolIcon.svelte';

  interface Props {
    activeTool: ToolId;
    lang: Language;
    onSelectTool: (tool: ToolId) => void;
    onOpenCommandPalette: () => void;
    onOpenSettings: () => void;
  }

  let {
    activeTool,
    lang = 'en',
    onSelectTool,
    onOpenCommandPalette,
    onOpenSettings,
  }: Props = $props();

  let t = $derived(translations[lang]);
  let tools = $derived(getLocalizedTools(lang));
  let primaryTools = $derived(tools.filter((it) => it.group === 'primary'));
  let secondaryTools = $derived(tools.filter((it) => it.group === 'secondary'));

  function handleNavKeydown(event: KeyboardEvent) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const index = tools.findIndex((it) => it.id === activeTool);
    if (index === -1) return;
    const nextIndex =
      event.key === 'ArrowDown'
        ? (index + 1) % tools.length
        : (index - 1 + tools.length) % tools.length;
    onSelectTool(tools[nextIndex].id);
  }
</script>

<aside class="sidebar" aria-label="Tools Navigation">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="titlebar-spacer drag-region"
    data-tauri-drag-region
    onmousedown={startWindowDrag}
    onpointerdown={startWindowDrag}
  ></div>

  <div class="palette-trigger-wrap no-drag">
    <button
      type="button"
      class="palette-trigger"
      onclick={onOpenCommandPalette}
      aria-label={`${t.switchTool} (Command+K)`}
    >
      <span class="palette-trigger-left">
        <Search size={14} strokeWidth={1.5} aria-hidden="true" />
        <span>{t.switchTool}</span>
      </span>
      <kbd class="kbd">⌘K</kbd>
    </button>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <nav class="nav-list no-drag" aria-label="PDF Tools" onkeydown={handleNavKeydown}>
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

  <div class="sidebar-footer no-drag">
    <button
      type="button"
      class="footer-btn"
      onclick={onOpenSettings}
      aria-label={t.settings}
      title={t.settings}
    >
      <Settings size={15} strokeWidth={1.5} />
      <span>{t.settings}</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-width);
    min-width: var(--sidebar-width);
    height: 100%;
    display: flex;
    flex-direction: column;
    background-color: var(--sidebar-surface);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border-right: 1px solid var(--border);
    box-shadow: inset -1px 0 0 0 rgba(255, 255, 255, 0.08);
  }

  .titlebar-spacer {
    height: var(--titlebar-height);
    flex-shrink: 0;
    cursor: grab;
  }

  .titlebar-spacer:active {
    cursor: grabbing;
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
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    cursor: pointer;
    box-shadow: var(--glass-glow), 0 1px 3px rgba(0, 0, 0, 0.04);
    transition:
      border-color var(--transition-fast),
      background-color var(--transition-fast),
      color var(--transition-fast);
  }

  .palette-trigger:hover {
    border-color: var(--accent);
    background-color: var(--surface-hover);
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
    background-color: var(--surface);
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
    opacity: 0.7;
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
      color var(--transition-fast),
      transform var(--transition-fast);
  }

  .nav-item:hover {
    background-color: var(--surface-hover);
    color: var(--text);
  }

  .nav-item.active {
    background-color: var(--accent-subtle);
    color: var(--accent);
    font-weight: 600;
    box-shadow: inset 0 0 0 1px rgba(129, 140, 248, 0.25);
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

  .sidebar-footer {
    padding: var(--space-2) var(--space-3);
    border-top: 1px solid var(--border);
  }

  .footer-btn {
    width: 100%;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 6px var(--space-2);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition:
      background-color var(--transition-fast),
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .footer-btn:hover {
    background-color: var(--surface);
    border-color: var(--border);
    color: var(--text);
  }
</style>
