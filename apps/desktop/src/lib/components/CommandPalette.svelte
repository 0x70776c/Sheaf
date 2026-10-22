<script lang="ts">
  import { FolderOpen, Moon, Sun, Monitor, Search, Settings, Globe } from '@lucide/svelte';
  import { getLocalizedTools } from '../tools';
  import type { ThemePreference, ToolId } from '../types';
  import type { Language } from '../i18n';
  import { translations } from '../i18n';
  import ToolIcon from './ToolIcon.svelte';

  interface Props {
    open: boolean;
    activeTool: ToolId;
    theme: ThemePreference;
    lang?: Language;
    onClose: () => void;
    onSelectTool: (tool: ToolId) => void;
    onOpenFiles: () => void;
    onSelectTheme: (theme: ThemePreference) => void;
    onOpenSettings?: () => void;
    onSelectLanguage?: (lang: Language) => void;
  }

  let {
    open,
    activeTool,
    theme,
    lang = 'en',
    onClose,
    onSelectTool,
    onOpenFiles,
    onSelectTheme,
    onOpenSettings,
    onSelectLanguage,
  }: Props = $props();

  let t = $derived(translations[lang]);
  let tools = $derived(getLocalizedTools(lang));

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  interface PaletteEntry {
    id: string;
    label: string;
    subtitle: string;
    shortcut?: string;
    kind: 'tool' | 'open' | 'theme' | 'settings' | 'lang';
    toolId?: ToolId;
    themeValue?: ThemePreference;
    langValue?: Language;
  }

  let allEntries = $derived.by<PaletteEntry[]>(() => [
    ...tools.map((it) => ({
      id: `tool:${it.id}`,
      label: it.label,
      subtitle: it.description,
      shortcut: it.shortcut,
      kind: 'tool' as const,
      toolId: it.id,
    })),
    {
      id: 'action:open',
      label: lang === 'de' ? 'PDF-Datei öffnen…' : 'Open PDF File…',
      subtitle: lang === 'de' ? 'Lokale PDF-Dateien auswählen' : 'Choose one or more local PDF files from disk',
      shortcut: '⌘O',
      kind: 'open',
    },
    {
      id: 'action:settings',
      label: t.settings,
      subtitle: lang === 'de' ? 'Sprache, Theme und Infos anpassen' : 'Language, appearance theme and info',
      shortcut: '⌘,',
      kind: 'settings',
    },
    {
      id: 'lang:en',
      label: 'Language: English',
      subtitle: 'Set interface language to English (default)',
      kind: 'lang',
      langValue: 'en',
    },
    {
      id: 'lang:de',
      label: 'Sprache: Deutsch',
      subtitle: 'Benutzeroberfläche auf Deutsch umstellen',
      kind: 'lang',
      langValue: 'de',
    },
    {
      id: 'theme:system',
      label: `${t.theme}: ${t.themeSystem}`,
      subtitle: lang === 'de' ? 'macOS-Erscheinungsbild automatisch folgen' : 'Follow macOS appearance automatically',
      kind: 'theme',
      themeValue: 'system',
    },
    {
      id: 'theme:light',
      label: `${t.theme}: ${t.themeLight}`,
      subtitle: lang === 'de' ? 'Helle Glaspalette nutzen' : 'Use bright frosted glass palette',
      kind: 'theme',
      themeValue: 'light',
    },
    {
      id: 'theme:dark',
      label: `${t.theme}: ${t.themeDark}`,
      subtitle: lang === 'de' ? 'Dunkle Glaspalette nutzen' : 'Use native dark glass palette',
      kind: 'theme',
      themeValue: 'dark',
    },
  ]);

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
      onClose();
    } else if (entry.kind === 'open') {
      onOpenFiles();
      onClose();
    } else if (entry.kind === 'theme' && entry.themeValue) {
      onSelectTheme(entry.themeValue);
      onClose();
    } else if (entry.kind === 'settings') {
      onOpenSettings?.();
      onClose();
    } else if (entry.kind === 'lang' && entry.langValue) {
      onSelectLanguage?.(entry.langValue);
      onClose();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      if (filteredEntries.length === 0) return;
      selectedIndex = (selectedIndex + 1) % filteredEntries.length;
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      if (filteredEntries.length === 0) return;
      selectedIndex =
        (selectedIndex - 1 + filteredEntries.length) % filteredEntries.length;
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      executeEntry(filteredEntries[selectedIndex]);
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="palette-backdrop no-drag" onclick={onClose}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div
      class="palette-modal"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      aria-label={t.switchTool}
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      <div class="palette-input-row">
        <Search size={16} strokeWidth={1.5} aria-hidden="true" />
        <input
          bind:this={inputEl}
          bind:value={query}
          type="text"
          class="palette-input"
          placeholder={t.switchTool}
          aria-label={t.switchTool}
        />
        <kbd class="esc-badge">ESC</kbd>
      </div>

      <div class="palette-results" role="listbox">
        {#if filteredEntries.length === 0}
          <div class="palette-empty">
            <span>{lang === 'de' ? 'Keine passenden Befehle gefunden' : 'No matching commands found'}</span>
          </div>
        {:else}
          {#each filteredEntries as item, index (item.id)}
            <button
              type="button"
              class="palette-item"
              class:selected={index === selectedIndex}
              role="option"
              aria-selected={index === selectedIndex}
              onclick={() => executeEntry(item)}
              onmouseenter={() => {
                selectedIndex = index;
              }}
            >
              <div class="item-icon" aria-hidden="true">
                {#if item.kind === 'tool' && item.toolId}
                  <ToolIcon tool={item.toolId} size={16} />
                {:else if item.kind === 'open'}
                  <FolderOpen size={16} strokeWidth={1.5} />
                {:else if item.kind === 'settings'}
                  <Settings size={16} strokeWidth={1.5} />
                {:else if item.kind === 'lang'}
                  <Globe size={16} strokeWidth={1.5} />
                {:else if item.themeValue === 'system'}
                  <Monitor size={16} strokeWidth={1.5} />
                {:else if item.themeValue === 'light'}
                  <Sun size={16} strokeWidth={1.5} />
                {:else}
                  <Moon size={16} strokeWidth={1.5} />
                {/if}
              </div>

              <div class="item-text">
                <span class="item-label">{item.label}</span>
                <span class="item-subtitle">{item.subtitle}</span>
              </div>

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
    background-color: rgba(0, 0, 0, 0.35);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 84px;
  }

  .palette-modal {
    width: 100%;
    max-width: 520px;
    background-color: var(--surface);
    backdrop-filter: blur(36px) saturate(190%);
    -webkit-backdrop-filter: blur(36px) saturate(190%);
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-card);
    box-shadow: var(--glass-glow), var(--shadow-floating);
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
    background-color: var(--surface);
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
    color: var(--text-muted);
  }

  .palette-item.selected .item-icon {
    color: var(--accent);
  }

  .item-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }

  .item-label {
    font-weight: 500;
    color: var(--text);
    font-size: 13px;
  }

  .item-subtitle {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-shortcut {
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--text-muted);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
  }
</style>
