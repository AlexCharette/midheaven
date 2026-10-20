<script lang="ts">
  // The gazetteer typeahead, lifted from BirthForm so the birth form and the
  // live calculator share one picker: query → suggestions dropdown → a picked
  // PlaceDto. A place is only ever `value` when picked from the suggestions
  // (typing clears it), preserving the "the id must round-trip to geo::by_id"
  // contract.
  import { searchPlaces } from "$lib/api";
  import type { PlaceDto } from "$lib/types";

  let {
    value = $bindable(null),
    placeholder = "type a city",
    compact = false,
    ariaLabel = "place",
    onpick = () => {},
  }: {
    value?: PlaceDto | null;
    placeholder?: string;
    /** The calculator's small inline variant — caption-sized, no form gutter. */
    compact?: boolean;
    ariaLabel?: string;
    onpick?: (p: PlaceDto) => void;
  } = $props();

  let query = $state(value?.label ?? "");
  let suggestions = $state<PlaceDto[]>([]);
  let sel = $state(0);

  // A parent may seed/replace the picked place (an `initial` prefill); mirror
  // its label into the input. Typing sets `value = null`, never a new place,
  // so this can't loop.
  $effect(() => {
    if (value && value.label !== query) query = value.label;
  });

  // monotonic counter: a slow stale response must not overwrite a newer one
  // or re-open a dropdown the user already resolved
  let latest = 0;
  async function queryPlaces() {
    value = null;
    const seq = ++latest;
    const q = query.trim();
    const result = q ? await searchPlaces(q) : [];
    if (seq === latest) {
      suggestions = result;
      sel = 0;
    }
  }

  function pick(p: PlaceDto) {
    latest++;
    value = p;
    query = p.label;
    suggestions = [];
    onpick(p);
  }

  function onKey(e: KeyboardEvent) {
    if (suggestions.length === 0) return;
    if (e.key === "ArrowDown") {
      sel = Math.min(sel + 1, suggestions.length - 1);
      e.preventDefault();
    } else if (e.key === "ArrowUp") {
      sel = Math.max(sel - 1, 0);
      e.preventDefault();
    } else if (e.key === "Enter") {
      pick(suggestions[sel]);
      e.preventDefault();
    } else if (e.key === "Escape") {
      // Closes the suggestions, and only them: the overlays listen for
      // Escape on the window, so letting it travel would shut the whole
      // sheet the picker sits in.
      suggestions = [];
      e.stopPropagation();
    }
  }
</script>

<span class="picker" class:compact>
  <input
    bind:value={query}
    oninput={queryPlaces}
    onkeydown={onKey}
    {placeholder}
    aria-label={ariaLabel}
  />
  {#if suggestions.length > 0}
    <ul class="dropdown">
      {#each suggestions as p, i (p.id)}
        <li>
          <button type="button" class:current={i === sel} onclick={() => pick(p)}>
            {p.label}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</span>

<style>
  /* the picker is its own positioning context, so the dropdown aligns to the
     input in any host layout (form gutter or caption line alike) */
  .picker {
    position: relative;
    display: inline-block;
  }
  .picker input {
    width: 100%;
  }
  .dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    min-width: 14rem;
    z-index: var(--z-dropdown);
    margin: 0.35rem 0 0;
    padding: 0.3rem;
    list-style: none;
    font-family: var(--font-sans);
    font-size: 0.86rem;
    background: var(--surface-raised);
    border-radius: var(--radius);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 12px 32px -8px rgba(0, 0, 0, 0.6);
  }
  .dropdown button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 0.42rem 0.6rem;
    border-radius: var(--radius-sm);
    color: var(--ink-2);
  }
  .dropdown button.current,
  .dropdown button:hover {
    color: var(--ink);
    background: var(--surface-hover);
  }
  /* caption variant: small-caps apparatus text, filling its line so long
     place labels never truncate */
  .compact {
    display: block;
    width: 100%;
  }
  .compact input {
    font-size: inherit;
    font-variant: small-caps;
    letter-spacing: 0.12em;
  }
  .compact .dropdown {
    font-variant: normal;
    letter-spacing: normal;
  }
</style>
