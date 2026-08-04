<script lang="ts">
  // Every note the app holds, in one place.
  //
  // Spoken and written alike, because they are the same thing — a view that
  // showed only the typed ones would revive a distinction the app does not
  // otherwise make. That has a cost worth knowing about: a few transcribed
  // readings outnumber the notes anyone typed, so the search field is not a
  // convenience here, it is how the view is used.
  //
  // Follows `Library.svelte`'s grammar deliberately: same rubric, same empty
  // plate, same manicule sliding in on a row. They are the two ways back into
  // work already done.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { deleteJournalNote, listJournal, loadChart } from "$lib/api";
  import { during, isBusy } from "$lib/busy.svelte";
  import { openReading as installReading, whyCannotOpenAnother } from "$lib/session.svelte";
  import { rowSubject, search } from "$lib/journal";
  import { setMoment, setOptions, setPlace } from "$lib/preview.svelte";
  import { toMinutes } from "$lib/civil";
  import { notify } from "$lib/toasts.svelte";
  import { chooseReadingsDir } from "$lib/readings";
  import { getPreferences } from "$lib/api";
  import type { JournalRow, RowSource } from "$lib/types";
  import { SIDEREAL, TROPICAL } from "$lib/types";
  import { reason } from "$lib/failure";

  let { onclose }: { onclose: () => void } = $props();

  let rows = $state<JournalRow[]>([]);
  let query = $state("");
  let loading = $state(true);
  let err = $state("");
  let readingsDir = $state<string | null>(null);

  const shown = $derived(search(rows, query));

  // once on mount (no reactive reads in the body)
  $effect(() => {
    refresh();
  });

  async function refresh() {
    loading = true;
    err = "";
    try {
      readingsDir = (await getPreferences()).readings_dir;
      rows = readingsDir ? await listJournal() : [];
    } catch (e) {
      err = reason(e);
    } finally {
      loading = false;
    }
  }

  async function choose() {
    if (await chooseReadingsDir()) refresh();
  }

  /** Open the reading a note was spoken or written in. Refused mid-take for
   * the same reason the library's is: it would re-seat the chart under a live
   * recorder. */
  async function openIts(source: Extract<RowSource, { kind: "reading" }>) {
    if (isBusy() || whyCannotOpenAnother()) return;
    err = "";
    try {
      const chart = await during("compute", () => loadChart(source.chartPath));
      if (installReading(chart)) notify(`opened ${chart.meta.name}'s reading`);
    } catch (e) {
      err = reason(e);
    }
  }

  /** Put the calculator back on the sky a note was written over. The seed is
   * all it takes — the chart was never stored, only what recomputes it. */
  function restoreIts(source: Extract<RowSource, { kind: "journal" }>) {
    const { seed, houseSystem, ayanamsa, locale, place } = source.anchor;
    const minutes = toMinutes(seed.date, seed.time);
    if (minutes === null) {
      err = "that note's moment cannot be read";
      return;
    }
    setPlace({ id: seed.place_id, label: place });
    setOptions({
      houseSystem,
      zodiac: ayanamsa ? SIDEREAL : TROPICAL,
      ayanamsa: ayanamsa ?? "",
      lang: locale,
    });
    setMoment(minutes, "now");
    onclose();
  }

  function jump(row: JournalRow) {
    if (row.source.kind === "reading") openIts(row.source);
    else restoreIts(row.source);
  }

  async function remove(ev: MouseEvent, row: JournalRow, id: string) {
    ev.stopPropagation();
    const sure = await ask(`Remove this note?\n\n${row.text}\n\nIt is not recoverable.`, {
      title: "Midheaven",
      kind: "warning",
    });
    if (!sure) return;
    try {
      await deleteJournalNote(id);
      rows = rows.filter((r) => !(r.source.kind === "journal" && r.source.id === id));
      notify("note removed");
    } catch (e) {
      err = reason(e);
    }
  }

  /** A note's identity across a re-search: its journal id, or its id within
   * the reading that holds it. Both are unique; neither is unique on its own. */
  const keyOf = (row: JournalRow) =>
    row.source.kind === "journal"
      ? `journal:${row.source.id}`
      : `${row.source.dir}:${row.source.noteId}`;

  const dayLabel = (iso: string) => {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "";
    const sameYear = d.getFullYear() === new Date().getFullYear();
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      ...(sameYear ? {} : { year: "numeric" }),
    });
  };
</script>

<div class="journal">
  <p class="rubric">everything written down</p>

  {#if loading}
    <p class="apparatus-text note">reading the journal…</p>
  {:else if err}
    <div class="empty-plate">
      <span class="mark err-mark" aria-hidden="true">✗</span>
      <p class="caption">The journal could not be read.</p>
      <p class="sub">{err}</p>
    </div>
  {:else if !readingsDir}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">No readings folder is set.</p>
      <p class="sub">Choose one to keep notes across your charts.</p>
      <button class="frame-btn" onclick={choose}>choose a folder…</button>
    </div>
  {:else if rows.length === 0}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">Nothing written down yet.</p>
      <p class="sub">Notes you write, and the words of readings you transcribe, appear here.</p>
    </div>
  {:else}
    <label class="find">
      <span class="apparatus-text">find</span>
      <input
        type="search"
        bind:value={query}
        placeholder="a word, a person, an element"
        aria-label="search notes"
      />
      <span class="apparatus-text count">{shown.length} of {rows.length}</span>
    </label>

    {#if shown.length === 0}
      <p class="apparatus-text note">Nothing here matches “{query}”.</p>
    {:else}
      <ul class="list">
        {#each shown as row (keyOf(row))}
          <li>
            <button
              type="button"
              class="row"
              onclick={() => jump(row)}
              disabled={row.source.kind === "reading" &&
                (isBusy() || whyCannotOpenAnother() !== null)}
              title={row.source.kind === "reading"
                ? (whyCannotOpenAnother() ?? "open this reading")
                : "put the calculator back on this sky"}
            >
              <span class="body">
                <span class="text">{row.text}</span>
                <span class="apparatus-text sub">
                  {rowSubject(row)}
                  {#each row.tagLabels as label}<span class="chip">{label}</span>{/each}
                </span>
              </span>
              <span class="meta apparatus-text">{dayLabel(row.writtenAt)}</span>
            </button>
            {#if row.source.kind === "journal"}
              {@const jid = row.source.id}
              <button
                type="button"
                class="remove"
                title="remove this note"
                aria-label="remove this note"
                onclick={(ev) => remove(ev, row, jid)}
              >
                ✕
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <p class="foot">
    <button type="button" class="ghost" onclick={onclose}>← back</button>
  </p>
</div>

<style>
  .journal {
    text-align: left;
    margin-top: 1rem;
  }
  .note {
    text-align: center;
    margin: 2.4rem 0;
  }
  .err-mark {
    color: var(--oxblood);
  }
  .find {
    display: flex;
    align-items: baseline;
    gap: 0.7rem;
    padding: 0.5rem 0.2rem 0.7rem;
  }
  .find input {
    flex: 1;
    min-width: 0;
    padding: 0.25rem 0.4rem;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--hairline);
    font: inherit;
  }
  .find input:focus-visible {
    outline: none;
    border-bottom-color: var(--brass);
  }
  .count {
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--line);
  }
  .list li {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--line);
  }
  .row {
    position: relative;
    flex: 1;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
    min-width: 0;
    padding: 0.7rem 0.4rem 0.7rem 1.5em;
    text-align: left;
    color: var(--ink-2);
    transition: background var(--dur-fast) var(--ease-out-quint);
  }
  /* the same manicule the library and the index point with */
  .row::before {
    content: "☞\FE0E";
    position: absolute;
    left: 0.3em;
    color: var(--brass);
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out-quint);
  }
  .row:disabled {
    cursor: wait;
  }
  .row:hover:not(:disabled) {
    color: var(--ink);
    background: var(--ink-a04);
  }
  .row:hover:not(:disabled)::before {
    opacity: 1;
  }
  @media (prefers-reduced-motion: reduce) {
    .row,
    .row::before {
      transition: none;
    }
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }
  /* two lines of the note, then an ellipsis — a row is an excerpt of itself */
  .text {
    color: var(--ink);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.35rem;
    font-size: 0.82rem;
  }
  .chip {
    border: 1px solid var(--hairline);
    padding: 0 0.35rem;
    color: var(--ink-3);
  }
  .meta {
    white-space: nowrap;
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }
  .remove {
    flex: none;
    padding: 0.3rem 0.55rem;
    margin-left: 0.4rem;
    color: var(--ink-3);
    font-size: 0.9rem;
  }
  .remove:hover {
    color: var(--oxblood);
  }
  .foot {
    margin-top: 1.2rem;
    text-align: center;
  }
</style>
