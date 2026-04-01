<script lang="ts">
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Card, CardContent } from "$lib/components/ui/card/index.js";
  import KeywordBadge from "./KeywordBadge.svelte";
  import { searchResults, searchQuery } from "$lib/stores.js";

  let results = $derived($searchResults ?? []);
  let query = $derived($searchQuery);
</script>

<div>
  <div class="mb-6">
    <h2 class="font-display text-lg font-bold tracking-wide">
      Search results
    </h2>
    <p class="text-sm text-[var(--muted-foreground)]">
      {results.length} clause{results.length !== 1 ? 's' : ''} matching "{query}"
    </p>
  </div>

  {#if results.length === 0}
    <div class="text-[var(--muted-foreground)] text-center py-12">
      No clauses match your search.
    </div>
  {:else}
    <div class="space-y-2">
      {#each results as result (result.clause_id)}
        <Card class="transition-colors hover:bg-[var(--accent)]/30">
          <CardContent class="p-4">
            <div class="flex gap-3 items-baseline">
              <KeywordBadge keyword={result.keyword} />
              <div class="flex-1 min-w-0">
                <p class="font-serif text-[16px] leading-relaxed">
                  {@html result.highlight}
                </p>
                {#if result.condition}
                  <div class="text-xs text-sky-400/80 font-mono mt-1">
                    GIVEN: {result.condition}
                  </div>
                {/if}
                <div class="flex items-center gap-3 mt-2 text-xs text-[var(--muted-foreground)]">
                  <span class="font-mono truncate">{result.spec_name}</span>
                  <span class="opacity-40">></span>
                  <span class="truncate">{result.section_path.split(' > ').slice(1).join(' > ')}</span>
                  {#if result.temporal}
                    <Badge variant="outline" class="text-[9px] bg-emerald-500/15 text-emerald-400 border-emerald-500/20">
                      {result.temporal.kind === 'invariant' ? 'INVARIANT' : result.temporal.duration}
                    </Badge>
                  {/if}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      {/each}
    </div>
  {/if}
</div>

<style>
  :global(mark) {
    background: var(--ring);
    color: var(--background);
    border-radius: 2px;
    padding: 0 2px;
  }
</style>
