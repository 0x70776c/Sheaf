<script lang="ts">
  import { Check } from '@lucide/svelte';
  import type { ThumbnailItem } from '../types';

  interface Props {
    totalPages: number;
    thumbnails: Map<number, ThumbnailItem>;
    selectedPages: Set<number>;
    loading: boolean;
    onSelectionChange: (nextSelected: Set<number>) => void;
    onRequestMoreThumbnails: (upToPage: number) => void;
  }

  let {
    totalPages,
    thumbnails,
    selectedPages,
    loading,
    onSelectionChange,
    onRequestMoreThumbnails,
  }: Props = $props();

  let anchorPage = $state<number | null>(null);

  let allNumbers = $derived(
    Array.from({ length: totalPages }, (_, idx) => idx + 1),
  );

  function handlePageClick(page: number, event: MouseEvent) {
    const next = new Set(selectedPages);

    if (event.shiftKey && anchorPage !== null) {
      const start = Math.min(anchorPage, page);
      const end = Math.max(anchorPage, page);
      if (!event.metaKey && !event.ctrlKey) {
        next.clear();
      }
      for (let p = start; p <= end; p++) {
        next.add(p);
      }
    } else if (event.metaKey || event.ctrlKey) {
      if (next.has(page)) {
        next.delete(page);
      } else {
        next.add(page);
      }
      anchorPage = page;
    } else {
      if (next.size === 1 && next.has(page)) {
        next.clear();
        anchorPage = null;
      } else {
        next.clear();
        next.add(page);
        anchorPage = page;
      }
    }

    onSelectionChange(next);
  }

  function handleSelectAll() {
    if (selectedPages.size === totalPages) {
      onSelectionChange(new Set());
      anchorPage = null;
    } else {
      onSelectionChange(new Set(allNumbers));
      anchorPage = 1;
    }
  }

  function handleClearSelection() {
    onSelectionChange(new Set());
    anchorPage = null;
  }

  function handleScroll(event: Event) {
    const container = event.currentTarget as HTMLElement;
    const nearBottom =
      container.scrollTop + container.clientHeight >=
      container.scrollHeight - 280;
    if (nearBottom && thumbnails.size < totalPages && !loading) {
      const nextTarget = Math.min(totalPages, thumbnails.size + 20);
      onRequestMoreThumbnails(nextTarget);
    }
  }
</script>

<div class="grid-container">
  <div class="grid-toolbar">
    <span class="selection-counter">
      {#if selectedPages.size === 0}
        {totalPages} {totalPages === 1 ? 'page' : 'pages'} · Click, ⇧-range, or ⌘-click to select
      {:else}
        <strong>{selectedPages.size}</strong> of {totalPages} selected
      {/if}
    </span>

    <div class="toolbar-actions">
      <button type="button" class="mini-btn" onclick={handleSelectAll}>
        {selectedPages.size === totalPages ? 'Deselect All' : 'Select All (⌘A)'}
      </button>
      {#if selectedPages.size > 0}
        <button type="button" class="mini-btn" onclick={handleClearSelection}>
          Clear
        </button>
      {/if}
    </div>
  </div>

  <div
    class="thumbnail-scroll"
    role="listbox"
    aria-multiselectable="true"
    aria-label="Document Pages"
    onscroll={handleScroll}
  >
    <div class="thumbnail-grid">
      {#each allNumbers as pageNum (pageNum)}
        {@const item = thumbnails.get(pageNum)}
        {@const isSelected = selectedPages.has(pageNum)}
        <button
          type="button"
          class="thumb-card"
          class:selected={isSelected}
          role="option"
          aria-selected={isSelected}
          aria-label={`Page ${pageNum}`}
          onclick={(e) => handlePageClick(pageNum, e)}
        >
          <div class="thumb-frame">
            {#if item}
              <img
                src={item.data_url}
                alt={`Page ${pageNum}`}
                width={item.width_px}
                height={item.height_px}
                loading="lazy"
              />
            {:else}
              <div class="thumb-skeleton" aria-hidden="true"></div>
            {/if}

            {#if isSelected}
              <span class="check-badge" aria-hidden="true">
                <Check size={12} strokeWidth={2} />
              </span>
            {/if}
          </div>

          <span class="thumb-label">{pageNum}</span>
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .grid-container {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .grid-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-6);
    border-bottom: 1px solid var(--border);
    background-color: var(--bg);
    font-size: 12px;
    color: var(--text-muted);
  }

  .selection-counter strong {
    color: var(--accent);
    font-weight: 600;
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .mini-btn {
    padding: 3px var(--space-2);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    transition:
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .mini-btn:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .thumbnail-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-6);
  }

  .thumbnail-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(136px, 1fr));
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
    cursor: pointer;
    transition:
      border-color var(--transition-fast),
      background-color var(--transition-fast),
      transform var(--transition-fast);
  }

  .thumb-card:hover {
    border-color: var(--accent);
  }

  .thumb-card.selected {
    border-color: var(--accent);
    background-color: var(--accent-subtle);
  }

  .thumb-frame {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .thumb-frame img {
    width: 100%;
    height: auto;
    border-radius: 4px;
    border: 1px solid var(--border);
    display: block;
    background-color: #ffffff;
  }

  .thumb-skeleton {
    width: 100%;
    aspect-ratio: 1 / 1.414;
    background-color: var(--border);
    border-radius: 4px;
    opacity: 0.55;
  }

  .check-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    background-color: var(--accent);
    color: #ffffff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .thumb-label {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .thumb-card.selected .thumb-label {
    color: var(--accent);
    font-weight: 600;
  }
</style>
