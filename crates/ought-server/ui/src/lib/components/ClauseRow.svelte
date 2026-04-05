<script lang="ts">
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import KeywordBadge from "./KeywordBadge.svelte";
  import CodeBlock from "./CodeBlock.svelte";
  import type { Clause } from "$lib/types.js";

  interface Props {
    clause: Clause;
  }

  let { clause }: Props = $props();
  let tests = $derived(clause.proofs?.tests ?? []);
  let hasProofs = $derived(tests.length > 0);

  let open = $state(false);
  let expanded = $state<Record<number, boolean>>({});

  function toggle() {
    if (hasProofs) open = !open;
  }

  function toggleCode(i: number) {
    expanded[i] = !expanded[i];
  }
</script>

<div class="flex gap-3 py-2 border-b border-[var(--border)]/50 items-baseline">
  <KeywordBadge keyword={clause.keyword} />
  <div class="flex-1 min-w-0">
    {#if clause.condition}
      <div class="text-xs text-sky-400/80 font-mono mb-1">
        GIVEN: {clause.condition}
      </div>
    {/if}
    <p class="font-serif text-[16px] leading-relaxed">
      {clause.text}
    </p>
    {#if clause.temporal}
      <span
        class="inline-block mt-1 text-[10px] font-semibold px-2 py-0.5 rounded bg-emerald-500/15 text-emerald-400 border border-emerald-500/20"
      >
        {clause.temporal.kind === "invariant"
          ? "INVARIANT"
          : clause.temporal.duration}
      </span>
    {/if}
    {#if clause.hints.length > 0}
      {#each clause.hints as hint}
        <pre
          class="mt-2 p-3 text-xs bg-[var(--muted)] rounded-md border font-mono overflow-x-auto">{hint}</pre>
      {/each}
    {/if}

    {#if hasProofs}
      <button
        type="button"
        onclick={toggle}
        class="mt-2 inline-flex items-center gap-1.5 text-[11px] font-mono text-[var(--muted-foreground)] hover:text-[var(--foreground)] cursor-pointer bg-transparent border-0 p-0"
      >
        <ChevronRight
          class="h-3 w-3 transition-transform {open ? 'rotate-90' : ''}"
        />
        <span class="uppercase tracking-wider">
          Proved by {tests.length} test{tests.length !== 1 ? "s" : ""}
        </span>
        {#if tests[0]?.summary && tests[0].summary !== clause.text}
          <span class="normal-case tracking-normal opacity-60 truncate">
            — {tests[0].summary}
          </span>
        {/if}
      </button>

      {#if open}
        <div class="mt-2 ml-4 space-y-2">
          {#if clause.proofs.file}
            <div class="text-[10px] font-mono text-[var(--muted-foreground)] opacity-60">
              {clause.proofs.file}
            </div>
          {/if}
          {#each tests as test, i}
            <div class="border border-[var(--border)] rounded-md overflow-hidden">
              <button
                type="button"
                onclick={() => toggleCode(i)}
                class="w-full flex items-center gap-2 px-3 py-2 text-left bg-[var(--muted)]/50 hover:bg-[var(--muted)] cursor-pointer border-0"
              >
                <ChevronRight
                  class="h-3 w-3 shrink-0 transition-transform {expanded[i]
                    ? 'rotate-90'
                    : ''}"
                />
                <span class="font-mono text-[11px] truncate">
                  {test.name}
                </span>
                {#if test.summary && test.summary !== test.name}
                  <span class="text-[11px] text-[var(--muted-foreground)] truncate italic">
                    — {test.summary}
                  </span>
                {/if}
              </button>
              {#if expanded[i]}
                <div class="p-2">
                  <CodeBlock code={test.code} language={test.language} />
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

{#if clause.otherwise.length > 0}
  <div class="ml-8 border-l border-dashed border-[var(--border)] pl-3">
    {#each clause.otherwise as ow}
      <div class="flex gap-3 py-2 items-baseline opacity-80">
        <KeywordBadge keyword={ow.keyword} />
        <p class="font-serif text-[16px] leading-relaxed">{ow.text}</p>
      </div>
    {/each}
  </div>
{/if}
