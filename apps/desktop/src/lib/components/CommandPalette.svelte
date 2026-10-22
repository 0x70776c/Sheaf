<script lang="ts">
  import { FolderOpen, Moon, Sun, Monitor, Search } from '@lucide/svelte';
  import { TOOLS } from '../tools';
  import type { ThemePreference, ToolId } from '../types';
  import ToolIcon from './ToolIcon.svelte';

  interface Props {
    open: boolean;
    activeTool: ToolId;
    theme: ThemePreference;
    onClose: () => void;
    onSelectTool: (tool: ToolId) => void;
    onOpenFiles: () => void;
    onSelectTheme: (theme: ThemePreference) => void;
    onOpenAbout?: () => void;
  }

  let {
    open,
    activeTool,
    theme,
    onClose,
    onSelectTool,
    onOpenFiles,
    onSelectTheme,
    onOpenAbout,
  }: Props = $props();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  interface PaletteEntry {
    id: string;
    label: string;
    subtitle: string;
    shortcut?: string;
    kind: 'tool' | 'open' | 'theme' | 'about';
    toolId?: ToolId;
    themeValue?: ThemePreference;
  }

  const allEntries: PaletteEntry[] = [
    ...TOOLS.map((t) => ({
      id: `tool:${t.id}`,
      label: t.label,
      subtitle: t.description,
      shortcut: t.shortcut,
      kind: 'tool' as const,
      toolId: t.id,
    })),
    {
      id: 'action:open',
      label: 'Open PDF File…',
      subtitle: 'Choose one or more local PDF files from disk',
      shortcut: '⌘O',
      kind: 'open',
    },
    {
      id: 'action:about',
      label: 'About Sheaf…',
      subtitle: 'Version 0.1.0, offline privacy statement and licenses',
      kind: 'about',
    },
    {
      id: 'theme:system',
      label: 'Theme: System Automatic',
      subtitle: 'Follow macOS appearance automatically',
      kind: 'theme',
      themeValue: 'system',
    },
    {
      id: 'theme:light',
      label: 'Theme: Light',
      subtitle: 'Use warm light palette (#FAFAF9)',
      kind: 'theme',
      themeValue: 'light',
    },
    {
      id: 'theme:dark',
      label: 'Theme: Dark',
      subtitle: 'Use native dark palette (#161617)',
      kind: 'theme',
      themeValue: 'dark',
    },
  ];

  let filteredEntries = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return allEntries;
    return allEntries.filter(
      (item) =>
        item.label.toLowerCase().includes(q) ||
        item.subtitle.toLowerCase().includes(q),
    );
  });

  $effect(() => {
    if (open) {
      query = '';
      selectedIndex = 0;
      queueMicrotask(() => {
        inputEl?.focus();
      });
    }
  });

  function executeEntry(entry: PaletteEntry | undefined) {
    if (!entry) return;
    if (entry.kind === 'tool' && entry.toolId) {
      onSelectTool(entry.toolId);
    } else if (entry.kind === 'open') {
      onOpenFiles();
    } else if (entry.kind === 'about') {
      onOpenAbout?.();
    } else if (entry.kind === 'theme' && entry.themeValue) {
      onSelectTheme(entry.themeValue);
    }
    onClose();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      if (filteredEntries.length > 0) {
        selectedIndex = (selectedIndex + 1) % filteredEntries.length;
      }
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      if (filteredEntries.length > 0) {
        selectedIndex =
          (selectedIndex - 1 + filteredEntries.length) % filteredEntries.length;
      }
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      executeEntry(filteredEntries[selectedIndex]);
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="palette-backdrop"
    onmousedown={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
    onkeydown={handleKeydown}
  >
    <div
      class="palette-modal"
      role="dialog"
      aria-modal="true"
      aria-label="Command Palette"
    >
      <div class="palette-input-row">
        <Search size={16} strokeWidth={1.5} aria-hidden="true" />
        <input
          bind:this={inputEl}
          type="text"
          class="palette-input"
          placeholder="Switch tool or run command…"
          aria-label="Search tools and commands"
          value={query}
          oninput={(e) => {
            query = e.currentTarget.value;
            selectedIndex = 0;
          }}
        />
        <kbd class="esc-badge">ESC</kbd>
      </div>

      <div class="palette-results" role="listbox" aria-label="Commands">
        {#if filteredEntries.length === 0}
          <div class="palette-empty">No matching commands found.</div>
        {:else}
          {#each filteredEntries as item, idx (item.id)}
            <button
              type="button"
              class="palette-item"
              class:selected={idx === selectedIndex}
              role="option"
              aria-selected={idx === selectedIndex}
              onmouseenter={() => {
                selectedIndex = idx;
              }}
              onclick={() => executeEntry(item)}
            >
              <span class="item-icon">
                {#if item.kind === 'tool' && item.toolId}
                  <ToolIcon tool={item.toolId} size={16} />
                {:else if item.kind === 'open'}
                  <FolderOpen size={16} strokeWidth={1.5} aria-hidden="true" />
                {:else if item.themeValue === 'dark'}
                  <Moon size={16} strokeWidth={1.5} aria-hidden="true" />
                {:else if item.themeValue === 'light'}
                  <Sun size={16} strokeWidth={1.5} aria-hidden="true" />
                {:else}
                  <Monitor size={16} strokeWidth={1.5} aria-hidden="true" />
                {/if}
              </span>

              <span class="item-text">
                <span class="item-title">
                  {item.label}
                  {#if item.kind === 'tool' && item.toolId === activeTool}
                    <span class="current-tag">Active</span>
                  {:else if item.kind === 'theme' && item.themeValue === theme}
                    <span class="current-tag">Current</span>
                  {/if}
                </span>
                <span class="item-subtitle">{item.subtitle}</span>
              </span>

              {#if item.shortcut}
                <kbd class="item-shortcut">{item.shortcut}</kbd>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .palette-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background-color: rgba(0, 0, 0, 0.28);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 84px;
  }

  .palette-modal {
    width: 100%;
    max-width: 520px;
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-floating);
    overflow: hidden;
  }

  .palette-input-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
  }

  .palette-input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: 14px;
    outline: none;
  }

  .esc-badge {
    font-size: 10px;
    color: var(--text-muted);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 2px 6px;
  }

  .palette-results {
    max-height: 340px;
    overflow-y: auto;
    padding: var(--space-2);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .palette-empty {
    padding: var(--space-6);
    text-align: center;
    color: var(--text-muted);
  }

  .palette-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border: none;
    border-radius: var(--radius-control);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--transition-fast);
  }

  .palette-item.selected {
    background-color: var(--accent-subtle);
  }

  .item-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }

  .palette-item.selected .item-icon {
    color: var(--accent);
  }

  .item-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .item-title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: 500;
    color: var(--text);
  }

  .current-tag {
    font-size: 10px;
    font-weight: 600;
    color: var(--accent);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 5px;
  }

  .item-subtitle {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-shortcut {
    font-size: 11px;
    color: var(--text-muted);
    background-color: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 6px;
  }
</style>
