<script lang="ts">
  // The house-system / zodiac / ayanamsa selector trio — one home for the
  // "sidereal opts into an ayanamsa" rule, previously duplicated between the
  // birth form and the reading view's caption. Two skins: the form's italic
  // gutter grid and the cartouche's small-caps line.
  import { ayanamsas, houseSystems } from "$lib/options.svelte";

  let {
    houseSystem = $bindable("whole-sign"),
    zodiac = $bindable("tropical"),
    ayanamsa = $bindable("lahiri"),
    disabled = false,
    variant = "form",
    onchange = () => {},
  }: {
    houseSystem?: string;
    zodiac?: string;
    ayanamsa?: string;
    disabled?: boolean;
    variant?: "form" | "caption";
    onchange?: () => void;
  } = $props();

  const uid = $props.id();
</script>

{#if variant === "form"}
  <div class="pair">
    <label class="lbl" for="{uid}-house">house system</label>
    <select id="{uid}-house" class="lang" bind:value={houseSystem} {disabled} {onchange}>
      {#each houseSystems as h (h.code)}
        <option value={h.code}>{h.label}</option>
      {/each}
    </select>
    <label class="lbl" for="{uid}-zodiac">zodiac</label>
    <select id="{uid}-zodiac" class="lang" bind:value={zodiac} {disabled} {onchange}>
      <option value="tropical">Tropical</option>
      <option value="sidereal">Sidereal</option>
    </select>
  </div>
  {#if zodiac === "sidereal"}
    <label class="aya">
      <span>ayanamsa</span>
      <select class="lang" bind:value={ayanamsa} {disabled} {onchange}>
        {#each ayanamsas as a (a.code)}
          <option value={a.code}>{a.label}</option>
        {/each}
      </select>
    </label>
  {/if}
{:else}
  <div class="calc">
    <select class="calc-sel" aria-label="house system" bind:value={houseSystem} {disabled} {onchange}>
      {#each houseSystems as h (h.code)}<option value={h.code}>{h.label}</option>{/each}
    </select>
    <span class="sep" aria-hidden="true">·</span>
    <select class="calc-sel" aria-label="zodiac" bind:value={zodiac} {disabled} {onchange}>
      <option value="tropical">Tropical</option>
      <option value="sidereal">Sidereal</option>
    </select>
    {#if zodiac === "sidereal"}
      <select class="calc-sel" aria-label="ayanamsa" bind:value={ayanamsa} {disabled} {onchange}>
        {#each ayanamsas as a (a.code)}<option value={a.code}>{a.label}</option>{/each}
      </select>
    {/if}
  </div>
{/if}

<style>
  /* --- form variant: mirrors the birth form's field grammar --- */
  /* house system + zodiac share a row, packed to the left: the first label
     keeps the shared 7.5rem gutter (aligning with every other field), each
     select sizes to its content, and "zodiac" sits snug after the first
     select rather than drifting to the plate edge. */
  .pair {
    display: grid;
    grid-template-columns: 7.5rem auto 4.5rem auto;
    justify-content: start;
    gap: 0 1rem;
    align-items: baseline;
    margin-bottom: 0.7rem;
  }
  .pair .lbl {
    font-style: italic;
    color: var(--ink-3);
    text-align: right;
  }
  .aya {
    display: grid;
    grid-template-columns: 7.5rem 1fr auto;
    gap: 0 1rem;
    align-items: baseline;
    margin-bottom: 0.7rem;
  }
  .aya span {
    font-style: italic;
    color: var(--ink-3);
    text-align: right;
  }
  .lang {
    justify-self: start;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--line);
    padding: 0.1rem 0.2rem;
    font: inherit;
  }
  .lang option {
    background: var(--bg-deep);
    color: var(--ink);
  }

  /* --- caption variant: the cartouche's live calc line — engraved,
     chromeless small-caps selects (ported from ChartCore) --- */
  .calc {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.1rem 0.3rem;
    font-size: inherit;
    font-variant: small-caps;
    letter-spacing: 0.12em;
    color: var(--ink-2);
  }
  .calc-sel {
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--line);
    color: var(--ink-2);
    font: inherit;
    font-variant: small-caps;
    letter-spacing: inherit;
    padding: 0 0.1rem;
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease-out-quint),
      border-color var(--dur-fast) var(--ease-out-quint);
  }
  .calc-sel:hover,
  .calc-sel:focus-visible {
    color: var(--ink);
    border-bottom-color: var(--hairline);
  }
  .calc-sel:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .calc option {
    background: var(--bg-deep);
    color: var(--ink);
    font-variant: normal;
    letter-spacing: normal;
  }
  .calc .sep {
    color: var(--ink-3);
  }
</style>
