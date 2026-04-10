<script lang="ts">
  import { onMount } from "svelte";
  import { data, activeSpecIndex, loadData, searchResults, searchQuery } from "$lib/stores.js";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import SpecView from "$lib/components/SpecView.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import SearchResults from "$lib/components/SearchResults.svelte";
  import ThemeToggle from "$lib/components/ThemeToggle.svelte";
  import { Separator } from "$lib/components/ui/separator/index.js";

  onMount(loadData);
</script>

<div class="flex flex-col h-screen bg-[var(--background)] text-[var(--foreground)] font-sans text-sm">
  <!-- Header -->
  <header class="flex items-center gap-4 px-5 h-12 border-b shrink-0">
    <h1 class="flex items-center gap-2 font-display text-sm font-bold tracking-widest uppercase">
      <img
        src="/ought-logo.svg"
        alt="ought"
        class="h-5 w-auto dark:invert"
      />
      <span class="opacity-40 font-normal">/ VIEWER</span>
    </h1>
    <div class="ml-auto flex items-center gap-3">
      {#if $data}
        <span class="text-xs text-[var(--muted-foreground)]"
          >{$data.stats.total_specs} specs</span
        >
        <span class="text-xs text-[var(--muted-foreground)]"
          >{$data.stats.total_clauses} clauses</span
        >
      {/if}
      <ThemeToggle />
    </div>
  </header>

  <SearchBar />

  <!-- Main layout -->
  <div class="flex flex-1 overflow-hidden">
    <Sidebar />
    <Separator orientation="vertical" />
    <main class="flex-1 overflow-y-auto p-8">
      {#if $searchQuery && $searchResults !== null}
        <SearchResults />
      {:else if $data}
        <SpecView />
      {:else}
        <p class="text-[var(--muted-foreground)] p-10">Loading specs...</p>
      {/if}
    </main>
  </div>
</div>
