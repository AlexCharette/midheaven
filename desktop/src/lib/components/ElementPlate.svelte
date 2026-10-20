<script lang="ts">
  // The vocabulary as a plate you can click, with no sky behind it.
  //
  // `Composer.svelte` states the rule this exists to keep: "the plate is how
  // you say what the note is about: while this is open the wheel and the
  // element index become an index — click Saturn, click the fifth house —
  // and the pins are the note's tags. A panel covering them would have
  // needed a second way to name elements in an app that has one."
  //
  // A standing note has no plate. Rather than give the app a second way to
  // name elements, this is the same plate minus the parts that needed a
  // chart: the bands are global (eleven bodies, twelve signs, twelve houses,
  // five kinds), only the wheel and the aspect pairs were ever chart-shaped.
  import { listElements } from "$lib/api";
  import { textGlyph } from "$lib/types";
  import type { ElementDto } from "$lib/types";
  import { onMount } from "svelte";

  let {
    selected,
    ontoggle,
  }: { selected: Set<string>; ontoggle: (tag: string) => void } = $props();

  let elements = $state<ElementDto[]>([]);

  onMount(async () => {
    elements = await listElements().catch(() => []);
  });

  // The band order the artifact and the reading view both use.
  const bands = $derived(
    (["planet", "sign", "house", "kind"] as const).map((cat) => ({
      cat,
      entries: elements.filter((e) => e.cat === cat),
    })),
  );
</script>

<div class="plate" role="group" aria-label="what this note is about">
  {#each bands as band (band.cat)}
    {#if band.entries.length > 0}
      <div class="band">
        <h4>{band.cat}s</h4>
        <div class="entries">
          {#each band.entries as e (e.tag)}
            <button
              type="button"
              class="entry"
              class:on={selected.has(e.tag)}
              data-cat={e.cat}
              aria-pressed={selected.has(e.tag)}
              title={e.name}
              onclick={() => ontoggle(e.tag)}
            >
              <span class="g">{textGlyph(e.glyph)}</span>
              <span class="nm">{e.name}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {/each}
</div>

<style>
  /* The same grammar as the reading view's Index of Elements: the category in
     a narrow gutter, its entries flowing beside it, a picked one holding the
     brass selection. */
  .plate {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.2rem 0 0;
    font-family: var(--font-sans);
  }
  .band {
    display: grid;
    grid-template-columns: 4.2rem 1fr;
    gap: 0 0.5rem;
    align-items: baseline;
    min-width: 0;
  }
  h4 {
    margin: 0;
    padding-top: 0.2rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--ink-3);
  }
  .entries {
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem;
    min-width: 0;
    margin-left: -0.45rem;
  }
  .entry {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35rem;
    padding: 0.16rem 0.45rem;
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-family: var(--font-sans);
    font-size: 0.8rem;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  .entry:hover {
    color: var(--ink);
    background: var(--surface-hover);
  }
  .entry.on {
    color: var(--ink);
    background: var(--brass-wash);
    box-shadow: inset 0 0 0 1px var(--brass-halo);
  }
  .entry:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 1px;
  }
  .entry .g {
    font-family: var(--font-astro);
    font-size: 1.05em;
  }
  /* The same category colouring the reading view and the artifact use. */
  .entry[data-cat="planet"] .g {
    color: var(--brass);
  }
  .entry[data-cat="sign"] .g {
    color: var(--verdigris);
  }
  .entry[data-cat="house"] .g {
    font-family: var(--font-serif);
    color: var(--steel);
    font-size: 0.9em;
  }
  .entry[data-cat="kind"] .g {
    color: var(--oxblood);
  }
  @media (prefers-reduced-motion: reduce) {
    .entry {
      transition: none;
    }
  }
</style>
