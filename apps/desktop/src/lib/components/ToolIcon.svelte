<script lang="ts">
  import {
    Layers,
    Scissors,
    LayoutGrid,
    Minimize2,
    Image,
    Stamp,
    Lock,
    Info,
  } from '@lucide/svelte';
  import type { ToolId } from '../types';

  interface Props {
    tool: ToolId;
    size?: number;
  }

  let { tool, size = 16 }: Props = $props();
</script>

{#if tool === 'merge'}
  <Layers {size} strokeWidth={1.5} aria-hidden="true" />
{:else if tool === 'split'}
  <Scissors {size} strokeWidth={1.5} aria-hidden="true" />
{:else if tool === 'pages'}
  <LayoutGrid {size} strokeWidth={1.5} aria-hidden="true" />
{:else if tool === 'compress'}
  <Minimize2 {size} strokeWidth={1.5} aria-hidden="true" />
{:else if tool === 'convert'}
  <Image {size} strokeWidth={1.5} aria-hidden="true" />
{:else if tool === 'watermark'}
  <Stamp {size} strokeWidth={1.5} aria-hidden="true" />
{:else if tool === 'protect'}
  <Lock {size} strokeWidth={1.5} aria-hidden="true" />
{:else}
  <Info {size} strokeWidth={1.5} aria-hidden="true" />
{/if}
