<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import Search from "lucide-svelte/icons/search";
  import Loader from "lucide-svelte/icons/loader";
  import { data, searchQuery, activeFilter, triggerSearch, isSearching } from "$lib/stores.js";
  import { KW_LABELS, KW_ORDER } from "$lib/types.js";

  const colorClass: Record<string, string> = {
    Must: "bg-red-500/15 text-red-500 border-red-500/20",
    MustNot: "bg-red-500/15 text-red-500 border-red-500/20",
    MustAlways: "bg-red-500/15 text-red-500 border-red-500/20",
    MustBy: "bg-red-500/15 text-red-500 border-red-500/20",
    Should: "bg-amber-500/15 text-amber-500 border-amber-500/20",
    ShouldNot: "bg-amber-500/15 text-amber-500 border-amber-500/20",
    May: "bg-zinc-500/15 text-zinc-400 border-zinc-500/20",
    Wont: "bg-violet-500/15 text-violet-400 border-violet-500/20",
    Given: "bg-sky-500/15 text-sky-400 border-sky-500/20",
    Otherwise: "bg-orange-500/15 text-orange-400 border-orange-500/20",
  };

  let keywords = $derived(
    $data
      ? KW_ORDER.filter((k) => ($data?.stats.by_keyword[k] ?? 0) > 0)
      : []
  );

  function toggleFilter(kw: string) {
    if ($activeFilter === kw) {
      activeFilter.set(null);
    } else {
      activeFilter.set(kw);
    }
  }

  function onInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchQuery.set(value);
    triggerSearch(value);
  }
</script>

<div
  class="px-5 py-2.5 bg-[var(--card)] border-b flex gap-2 items-center flex-wrap"
>
  {#if $isSearching}
    <Loader class="h-4 w-4 text-[var(--muted-foreground)] shrink-0 animate-spin" />
  {:else}
    <Search class="h-4 w-4 text-[var(--muted-foreground)] shrink-0" />
  {/if}
  <Input
    placeholder="Search all clauses..."
    class="flex-1 min-w-[200px] h-8 text-sm"
    value={$searchQuery}
    oninput={onInput}
  />
  <div class="flex gap-1 flex-wrap">
    {#each keywords as kw (kw)}
      <button
        class="border-0 bg-transparent p-0 cursor-pointer"
        onclick={() => toggleFilter(kw)}
      >
        <Badge
          variant="outline"
          class="text-[10px] font-semibold uppercase tracking-wider rounded-md transition-opacity cursor-pointer {colorClass[kw] ??
            ''} {$activeFilter === kw
            ? 'opacity-100 ring-1 ring-current'
            : 'opacity-45 hover:opacity-75'}"
        >
          {KW_LABELS[kw] ?? kw}
        </Badge>
      </button>
    {/each}
  </div>
</div>
