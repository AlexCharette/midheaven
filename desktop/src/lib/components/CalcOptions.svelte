<script lang="ts">
  // The house-system / zodiac / ayanamsa selector trio — one home for the
  // "sidereal opts into an ayanamsa" rule, previously duplicated between the
  // birth form and the reading view's caption. Three skins: the birth form's
  // labelled fields, the calculator's and the plate's compact controls, and
  // the older small-caps caption line.
  import { ayanamsas, defaults, houseSystems } from "$lib/options.svelte";
  import { SIDEREAL, TROPICAL } from "$lib/types";

  let {
    houseSystem = $bindable(defaults().houseSystem),
    zodiac = $bindable(defaults().zodiac),
    ayanamsa = $bindable(defaults().ayanamsa),
    disabled = false,
    variant = "form",
    onchange = () => {},
  }: {
    houseSystem?: string;
    zodiac?: string;
    ayanamsa?: string;
    disabled?: boolean;
    variant?: "form" | "caption" | "control";
    /** The whole trio as it now stands, so a parent that owns the choices
     * elsewhere can take them without two-way binding. */
    onchange?: (next: { houseSystem: string; zodiac: string; ayanamsa: string }) => void;
  } = $props();

  const report = () => onchange({ houseSystem, zodiac, ayanamsa });

  const uid = $props.id();
</script>

{#if variant === "form"}
  <!-- Bare fields with no wrapper of their own: the host's `.form .row` lays
       them out beside whatever else shares the line (the birth form puts the
       reading language there). -->
  <label class="fld">
    <span class="lbl">House system</span>
    <select bind:value={houseSystem} {disabled} onchange={report}>
      {#each houseSystems as h (h.code)}
        <option value={h.code}>{h.label}</option>
      {/each}
    </select>
  </label>
  <label class="fld">
    <span class="lbl">Zodiac</span>
    <select bind:value={zodiac} {disabled} onchange={report}>
      <option value={TROPICAL}>Tropical</option>
      <option value={SIDEREAL}>Sidereal</option>
    </select>
  </label>
  {#if zodiac === SIDEREAL}
    <label class="fld">
      <span class="lbl">Ayanamsa</span>
      <select bind:value={ayanamsa} {disabled} onchange={report}>
        {#each ayanamsas as a (a.code)}
          <option value={a.code}>{a.label}</option>
        {/each}
      </select>
    </label>
  {/if}
{:else if variant === "control"}
  <div class="ctl">
    <select aria-label="house system" bind:value={houseSystem} {disabled} onchange={report}>
      {#each houseSystems as h (h.code)}<option value={h.code}>{h.label}</option>{/each}
    </select>
    <select aria-label="zodiac" bind:value={zodiac} {disabled} onchange={report}>
      <option value={TROPICAL}>Tropical</option>
      <option value={SIDEREAL}>Sidereal</option>
    </select>
    {#if zodiac === SIDEREAL}
      <select aria-label="ayanamsa" bind:value={ayanamsa} {disabled} onchange={report}>
        {#each ayanamsas as a (a.code)}<option value={a.code}>{a.label}</option>{/each}
      </select>
    {/if}
  </div>
{:else}
  <div class="calc">
    <select class="calc-sel" aria-label="house system" bind:value={houseSystem} {disabled} onchange={report}>
      {#each houseSystems as h (h.code)}<option value={h.code}>{h.label}</option>{/each}
    </select>
    <span class="sep" aria-hidden="true">·</span>
    <select class="calc-sel" aria-label="zodiac" bind:value={zodiac} {disabled} onchange={report}>
      <option value={TROPICAL}>Tropical</option>
      <option value={SIDEREAL}>Sidereal</option>
    </select>
    {#if zodiac === SIDEREAL}
      <select class="calc-sel" aria-label="ayanamsa" bind:value={ayanamsa} {disabled} onchange={report}>
        {#each ayanamsas as a (a.code)}<option value={a.code}>{a.label}</option>{/each}
      </select>
    {/if}
  </div>
{/if}

<style>
  /* --- control variant: the calculator's apparatus — the choices as quiet
     filled controls in the sans voice, each its own native menu (the platform
     menu is the right affordance for a pick-one list; only the closed control
     is ours), with a drawn chevron in place of the platform's --- */
  .ctl {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .ctl select {
    appearance: none;
    -webkit-appearance: none;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    font-weight: 450;
    color: var(--ink-2);
    background-color: var(--surface);
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath d='M1 1l4 3.8L9 1' fill='none' stroke='%239a957f' stroke-width='1.3' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.6rem center;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0.32rem 1.7rem 0.32rem 0.65rem;
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease-out-quint),
      background-color var(--dur-fast) var(--ease-out-quint),
      border-color var(--dur-fast) var(--ease-out-quint);
  }
  .ctl select:hover:not(:disabled) {
    color: var(--ink);
    background-color: var(--surface-hover);
    border-color: var(--hairline);
  }
  .ctl select:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 1px;
  }
  .ctl select:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .ctl option {
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
