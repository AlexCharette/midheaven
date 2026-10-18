<script lang="ts">
  // ─── SPIKE: the ledger ─────────────────────────────────────────────────
  //
  // Every note the app holds, as one account book rather than a panel you
  // search. The Journal overlay answers "where is that note"; this answers
  // "what have I been writing", which is a different question and wants a
  // different shape: one column, dense, chronological, ruled by the day.
  //
  // It carries what the panel carried — the find box, the person filter —
  // because the answer to the third question was that it replaces it. What
  // it does not carry is the veil: a ledger is a place you are in, not a
  // thing over something else.
  import { onMount } from "svelte";
  import { listPeople, queryNotes } from "$lib/api";
  import { search } from "$lib/journal";
  import { reason } from "$lib/failure";
  import type { JournalRow, Person, Query } from "$lib/types";

  let rows = $state<JournalRow[]>([]);
  let people = $state<Person[]>([]);
  let filter = $state<Query>({ tags: [], mode: "any" });
  let find = $state("");
  let loading = $state(true);
  let err = $state("");

  const shown = $derived(search(rows, find));

  /** Rows grouped under the day they were written — the ledger's rule. */
  const days = $derived.by(() => {
    const out: { key: string; label: string; rows: JournalRow[] }[] = [];
    for (const r of shown) {
      const d = new Date(r.writtenAt);
      const key = Number.isNaN(d.getTime()) ? "undated" : d.toDateString();
      const last = out.at(-1);
      if (last?.key === key) last.rows.push(r);
      else out.push({ key, label: dayLabel(d), rows: [r] });
    }
    return out;
  });

  // The app's chrome is English until its own locale says otherwise; passing
  // `undefined` here hands the decision to the operating system, which is how
  // an English page ends up with German weekdays. (`Journal.svelte` and
  // `Commentary.svelte` have the same latent bug.)
  const UI_LOCALE = "en";

  function dayLabel(d: Date): string {
    if (Number.isNaN(d.getTime())) return "no date";
    const sameYear = d.getFullYear() === new Date().getFullYear();
    return d.toLocaleDateString(UI_LOCALE, {
      weekday: "long",
      month: "long",
      day: "numeric",
      ...(sameYear ? {} : { year: "numeric" }),
    });
  }

  const clock = (iso: string) => {
    const d = new Date(iso);
    return Number.isNaN(d.getTime())
      ? "—"
      : d.toLocaleTimeString(UI_LOCALE, { hour: "2-digit", minute: "2-digit", hour12: false });
  };

  onMount(() => void load());

  async function load() {
    loading = true;
    err = "";
    try {
      [rows, people] = await Promise.all([queryNotes(filter), listPeople()]);
    } catch (e) {
      err = reason(e);
    } finally {
      loading = false;
    }
  }

  async function byPerson(id: string) {
    filter = id ? { ...filter, person: id } : { ...filter, person: undefined };
    await load();
  }
</script>

<div class="ledger">
  <header>
    <h1>the ledger</h1>
    <label class="who">
      <span class="apparatus-text">about</span>
      <select
        value={filter.person ?? ""}
        onchange={(e) => byPerson(e.currentTarget.value)}
        aria-label="narrow to one person"
      >
        <option value="">everyone</option>
        {#each people as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
      </select>
    </label>
    <label class="find">
      <span class="apparatus-text">find</span>
      <input type="search" bind:value={find} placeholder="a word, a person, an element" />
    </label>
    <span class="tally apparatus-text">{shown.length} of {rows.length}</span>
  </header>

  {#if loading}
    <p class="apparatus-text state">reading the ledger…</p>
  {:else if err}
    <p class="state fail">{err}</p>
  {:else if rows.length === 0}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">Nothing written down yet.</p>
      <p class="sub">Notes you write, and the words of readings you transcribe, appear here.</p>
    </div>
  {:else if shown.length === 0}
    <p class="apparatus-text state">Nothing here matches “{find}”.</p>
  {:else}
    {#each days as day (day.key)}
      <section class="day">
        <h2>{day.label}</h2>
        {#each day.rows as row (row.id)}
          <article class="entry">
            <span class="clock">{clock(row.writtenAt)}</span>
            <p class="words">{row.text}</p>
            <p class="hand">
              {#if row.person}<span class="who-nm">{row.person.name}</span>{/if}
              {#each row.tagLabels as label}<span class="tag">{label}</span>{/each}
            </p>
          </article>
        {/each}
      </section>
    {/each}
  {/if}
</div>

<style>
  .ledger {
    max-width: 58rem;
    margin: 0 auto;
    padding: clamp(1.4rem, 4vh, 3rem) clamp(1.2rem, 4vw, 2.6rem) 5rem;
  }
  header {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 0.6rem 1.4rem;
    padding-bottom: 0.7rem;
    border-bottom: 1px solid var(--hairline);
  }
  h1 {
    margin: 0 auto 0 0;
    font-weight: 400;
    font-size: 1.1rem;
    font-variant: small-caps;
    letter-spacing: 0.26em;
  }
  .who,
  .find {
    display: inline-flex;
    align-items: baseline;
    gap: 0.45rem;
  }
  .find input {
    min-width: 14rem;
  }
  .tally {
    font-variant-numeric: tabular-nums;
  }
  /* The day is the ledger's rule: a dated head, then its entries. */
  .day {
    margin-top: 1.8rem;
  }
  .day h2 {
    margin: 0 0 0.2rem;
    font-weight: 400;
    font-size: 0.76rem;
    font-variant: small-caps;
    letter-spacing: 0.2em;
    color: var(--ink-3);
  }
  /* An account book's row: the hour in a narrow gutter, the words in the
     column, what it was filed under beneath them. Ruled, never boxed. */
  .entry {
    display: grid;
    grid-template-columns: 3.4rem 1fr;
    gap: 0.1rem 1.1rem;
    padding: 0.7rem 0;
    border-top: 1px solid var(--line);
    user-select: text;
    cursor: text;
  }
  .clock {
    grid-row: 1 / span 2;
    color: var(--ink-3);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    text-align: right;
    padding-top: 0.2rem;
  }
  .words {
    grid-column: 2;
    margin: 0;
    line-height: 1.62;
    text-wrap: pretty;
  }
  .hand {
    grid-column: 2;
    margin: 0.2rem 0 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 0.7rem;
    font-size: 0.8rem;
    color: var(--ink-3);
  }
  .who-nm {
    font-style: italic;
    color: var(--ink-2);
  }
  .tag {
    color: var(--ink-3);
  }
  .tag::before {
    content: "·";
    margin-right: 0.45rem;
    opacity: 0.6;
  }
  .state {
    margin: 2.4rem 0;
  }
  .fail {
    color: var(--oxblood);
    font-style: italic;
  }
  /* Narrow: the hour goes above the words, as it does in the commentary —
     a fixed gutter at 390px starves the measure. */
  @media (max-width: 560px) {
    .entry {
      grid-template-columns: 1fr;
      gap: 0.15rem;
    }
    .clock {
      grid-row: auto;
      text-align: left;
      padding-top: 0;
    }
    .words,
    .hand {
      grid-column: 1;
    }
    .find input {
      min-width: 0;
    }
  }
</style>
