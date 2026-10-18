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
        <h4 class="apparatus-text">{band.cat}s</h4>
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
  .plate {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.2rem 0 0;
  }
  .band {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  h4 {
    margin: 0;
    font-size: 0.6rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-3);
    font-weight: 400;
  }
  .entries {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem 0.3rem;
  }
  .entry {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3rem;
    background: none;
    border: 1px solid transparent;
    border-radius: 2px;
    color: var(--ink-2);
    font: inherit;
    font-size: 0.82rem;
    padding: 0.08rem 0.35rem;
    cursor: pointer;
  }
  .entry:hover {
    color: var(--ink);
    border-color: var(--line);
  }
  .entry.on {
    color: var(--ink);
    border-color: var(--hairline);
    background: var(--plate-inset, rgba(255, 255, 255, 0.04));
  }
  .entry .g {
    font-size: 0.9em;
  }
  /* The same category colouring the reading view and the artifact use. */
  .entry[data-cat="planet"] .g {
    color: var(--brass);
  }
  .entry[data-cat="sign"] .g {
    color: var(--verdigris);
  }
  .entry[data-cat="house"] .g {
    color: var(--steel);
    font-size: 0.8em;
    letter-spacing: 0.06em;
  }
  .entry[data-cat="kind"] .g {
    color: var(--oxblood);
  }
</style>
