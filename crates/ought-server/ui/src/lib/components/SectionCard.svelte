<script lang="ts">
  import { Card, CardHeader, CardContent } from "$lib/components/ui/card/index.js";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import ClauseRow from "./ClauseRow.svelte";
  import SectionCard from "./SectionCard.svelte";
  import type { Section } from "$lib/types.js";
  import { activeFilter, filterClauses } from "$lib/stores.js";

  interface Props {
    section: Section;
    depth?: number;
  }

  let { section, depth = 0 }: Props = $props();

  let open = $state(true);

  let filteredClauses = $derived(
    filterClauses(section.clauses, $activeFilter)
  );

  function toggleOpen() {
    open = !open;
  }
</script>

{#if depth === 0}
  <Card class="mb-3">
    <button
      class="w-full flex flex-row items-center gap-2 py-3 px-4 cursor-pointer hover:bg-[var(--accent)]/50 rounded-t-lg border-0 bg-transparent text-inherit text-left"
      onclick={toggleOpen}
    >
      <ChevronRight
        class="h-4 w-4 text-[var(--muted-foreground)] transition-transform {open
          ? 'rotate-90'
          : ''}"
      />
      <span class="font-semibold text-sm">{section.title}</span>
      <span class="ml-auto text-xs text-[var(--muted-foreground)] tabular-nums">
        {section.clauses.length} clauses
      </span>
    </button>
    {#if open}
      <CardContent class="pt-0 px-4 pb-3">
        {#if section.prose}
          <p
            class="text-sm text-[var(--muted-foreground)] mb-3 p-3 bg-[var(--muted)] rounded-md border-l-2 border-[var(--border)] italic"
          >
            {section.prose}
          </p>
        {/if}
        {#each filteredClauses as clause (clause.id)}
          <ClauseRow {clause} />
        {/each}
        {#each section.subsections as sub (sub.title)}
          <SectionCard section={sub} depth={depth + 1} />
        {/each}
      </CardContent>
    {/if}
  </Card>
{:else}
  <div class="ml-3 border-l border-[var(--border)] pl-4 mt-3">
    <h4 class="font-semibold text-[13px] mb-2">{section.title}</h4>
    {#if section.prose}
      <p class="text-sm text-[var(--muted-foreground)] mb-2 italic">
        {section.prose}
      </p>
    {/if}
    {#each filteredClauses as clause (clause.id)}
      <ClauseRow {clause} />
    {/each}
    {#each section.subsections as sub (sub.title)}
      <SectionCard section={sub} depth={depth + 1} />
    {/each}
  </div>
{/if}
