<script lang="ts">
  // Every note the app holds, in one place.
  //
  // Spoken and written alike, because they are the same thing — a view that
  // showed only the typed ones would revive a distinction the app does not
  // otherwise make. That has a cost worth knowing about: a few transcribed
  // readings outnumber the notes anyone typed, so the search field is not a
  // convenience here, it is how the view is used.
  //
  // A row used to carry a `source` saying which of two stores it came from,
  // and every control here branched on it. There is one store now: a row is
  // a note, and the chart it names is either someone's or a moment nobody
  // was consulting over.
  //
  // Follows `Library.svelte`'s grammar deliberately: same rubric, same empty
  // plate, same manicule sliding in on a row. They are the two ways back into
  // work already done.
  import { ask } from "@tauri-apps/plugin-dialog";
  import ElementPlate from "./ElementPlate.svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    deleteNote,
    deleteView,
    listPeople,
    listViews,
    openChart,
    queryNotes,
    writeStandingNote,
    writeView,
  } from "$lib/api";
  import { during, isBusy } from "$lib/busy.svelte";
  import { openReading as installReading, whyCannotOpenAnother } from "$lib/session.svelte";
  import { rowMark, rowSubject, search } from "$lib/journal";
  import { setMoment, setOptions, setPlace } from "$lib/preview.svelte";
  import { toMinutes } from "$lib/civil";
  import { notify } from "$lib/toasts.svelte";
  import { chooseReadingsDir } from "$lib/readings";
  import { getPreferences } from "$lib/api";
  import type { Chart, JournalRow, Person, Query, View } from "$lib/types";
  import { SIDEREAL, TROPICAL } from "$lib/types";
  import { refreshNotes } from "$lib/notes.svelte";
  import { reason } from "$lib/failure";

  let { onclose }: { onclose: () => void } = $props();

  let rows = $state<JournalRow[]>([]);
  let query = $state("");
  let loading = $state(true);
  let err = $state("");
  let readingsDir = $state<string | null>(null);

  // The standing question. The `find` box below is deliberately *not* part of
  // it: a view is something kept, and the box is a glance — it also searches
  // the person and the element labels, which the store cannot be asked about
  // without reading everything the row already carries.
  let filter = $state<Query>({ tags: [], mode: "any" });
  let people = $state<Person[]>([]);
  let views = $state<View[]>([]);
  let naming = $state(false);
  let viewName = $state("");

  // Writing a standing note. Whether it is craft or an observation about
  // someone falls out of the person selector rather than being a third thing
  // to say: name nobody and it is about the elements alone.
  let composing = $state(false);
  let draft = $state("");
  let draftTags = new SvelteSet<string>();
  let draftPerson = $state("");
  let saving = $state(false);

  const filtered = $derived(filter.person !== undefined || filter.tags.length > 0);
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
      if (!readingsDir) {
        rows = [];
        return;
      }
      [rows, people, views] = await Promise.all([
        queryNotes(filter),
        listPeople(),
        listViews(),
      ]);
    } catch (e) {
      err = reason(e);
    } finally {
      loading = false;
    }
  }

  /** Narrow to one element — what clicking a chip does, anywhere. */
  async function byTag(tag: string) {
    filter = { ...filter, tags: tag ? [tag] : [], mode: "any" };
    await refresh();
  }

  /** Narrow to one person, or to everyone. */
  async function byPerson(id: string) {
    filter = id ? { ...filter, person: id } : { ...filter, person: undefined };
    await refresh();
  }

  /** Keep the current question under a name. */
  async function keepView() {
    const name = viewName.trim();
    if (!name) return;
    try {
      await writeView(name, filter);
      viewName = "";
      naming = false;
      views = await listViews();
      notify(`kept “${name}”`);
    } catch (e) {
      err = reason(e);
    }
  }

  function beginCompose() {
    composing = true;
    draft = "";
    draftTags.clear();
    draftPerson = "";
  }

  async function keepNote() {
    const words = draft.trim();
    if (!words) {
      err = "a note needs words";
      return;
    }
    saving = true;
    err = "";
    try {
      await writeStandingNote(words, [...draftTags], draftPerson || undefined);
      composing = false;
      draft = "";
      draftTags.clear();
      await refresh();
      notify(draftPerson ? "note kept about them" : "standing note kept");
    } catch (e) {
      err = reason(e);
    } finally {
      saving = false;
    }
  }

  async function applyView(v: View) {
    filter = v.query;
    await refresh();
  }

  async function dropView(ev: MouseEvent, v: View) {
    ev.stopPropagation();
    try {
      await deleteView(v.id);
      views = views.filter((x) => x.id !== v.id);
    } catch (e) {
      err = reason(e);
    }
  }

  async function choose() {
    if (await chooseReadingsDir()) refresh();
  }

  /** Open the reading a note belongs to. Refused mid-take for the same
   * reason the library's is: it would re-seat the chart under a live
   * recorder. */
  async function openIts(id: string) {
    if (isBusy() || whyCannotOpenAnother()) return;
    err = "";
    try {
      const chart = await during("compute", () => openChart(id));
      if (installReading(chart)) {
        await refreshNotes();
        notify(`opened ${chart.meta.name}'s reading`);
      }
    } catch (e) {
      err = reason(e);
    }
  }

  /** Put the calculator back on the sky a note was written over. The seed is
   * all it takes — the chart was never stored, only what recomputes it. */
  function restoreIts(chart: Chart) {
    const { seed, houseSystem, ayanamsa, locale, place } = chart;
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

  /** Only a note said in someone's reading reopens one. */
  const isReading = (row: JournalRow) =>
    row.anchor.kind === "chart" && row.person !== undefined;

  /** What clicking a row does, which is different for each of the three
   * kinds — and in every case is "take me to the rest of this".
   *
   * A reading note reopens the reading. A note over a moment nobody was
   * consulting puts the calculator back on that sky. A note about a person
   * or about the elements has no sky to go to, so it narrows the journal to
   * the rest of its kind — which is the only "more of this" there is. */
  async function jump(row: JournalRow) {
    switch (row.anchor.kind) {
      case "chart":
        if (row.chart === undefined) return;
        if (isReading(row)) await openIts(row.chart.id);
        else restoreIts(row.chart);
        return;
      case "person":
        await byPerson(row.anchor.id);
        return;
      case "standing":
        filter = { ...filter, tags: [...row.tags], mode: "any", person: undefined };
        await refresh();
    }
  }

  async function remove(ev: MouseEvent, row: JournalRow, id: string) {
    ev.stopPropagation();
    const sure = await ask(`Remove this note?\n\n${row.text}\n\nIt is not recoverable.`, {
      title: "Midheaven",
      kind: "warning",
    });
    if (!sure) return;
    try {
      await deleteNote(id);
      rows = rows.filter((r) => r.id !== id);
      notify("note removed");
    } catch (e) {
      err = reason(e);
    }
  }

  // A note's id is unique across the whole store, so it is the key.

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
  {:else if rows.length === 0 && !filtered}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">Nothing written down yet.</p>
      <p class="sub">Notes you write, and the words of readings you transcribe, appear here.</p>
    </div>
  {:else}
    {#if composing}
      <div class="composing">
        <textarea
          bind:value={draft}
          rows="3"
          placeholder="what you know — about Saturn, about Leo, about someone"
          aria-label="the note"
        ></textarea>
        <ElementPlate
          selected={draftTags}
          ontoggle={(tag) => (draftTags.has(tag) ? draftTags.delete(tag) : draftTags.add(tag))}
        />
        <div class="compose-foot">
          <label class="who-filter">
            <span class="apparatus-text">about</span>
            <select bind:value={draftPerson} aria-label="who this note is about">
              <option value="">no one — the elements</option>
              {#each people as p (p.id)}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
          </label>
          <span class="grow"></span>
          <button type="button" class="ghost" onclick={() => (composing = false)}>cancel</button>
          <button type="button" class="frame-btn" disabled={saving} onclick={keepNote}>
            {saving ? "keeping…" : "keep"}
          </button>
        </div>
      </div>
    {/if}

    <div class="asking">
      <label class="who-filter">
        <span class="apparatus-text">about</span>
        <select
          value={filter.person ?? ""}
          onchange={(e) => byPerson(e.currentTarget.value)}
          aria-label="narrow to one person"
        >
          <option value="">everyone</option>
          {#each people as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      </label>

      {#if views.length > 0}
        <span class="views">
          {#each views as v (v.id)}
            <span class="view-chip">
              <button type="button" class="chip-open" onclick={() => applyView(v)} title="ask this again"
                >{v.name}</button
              >
              <button
                type="button"
                class="chip-drop"
                onclick={(ev) => dropView(ev, v)}
                title="forget this view"
                aria-label={`forget ${v.name}`}>✕</button
              >
            </span>
          {/each}
        </span>
      {/if}

      {#if filter.tags.length > 0}
        <span class="narrowed apparatus-text">
          filed under
          {#each filter.tags as t}<span class="chip-static">{t}</span>{/each}
          <button type="button" class="ghost" onclick={() => byTag("")}>clear</button>
        </span>
      {/if}

      {#if !composing}
        <button type="button" class="ghost" onclick={beginCompose}>write a note…</button>
      {/if}

      {#if naming}
        <span class="naming">
          <input
            type="text"
            bind:value={viewName}
            placeholder="call it something"
            aria-label="name for this view"
            onkeydown={(e) => {
              if (e.key === "Enter") keepView();
              if (e.key === "Escape") { naming = false; viewName = ""; }
            }}
          />
          <button type="button" class="ghost" onclick={keepView}>keep</button>
        </span>
      {:else if filtered}
        <button type="button" class="ghost" onclick={() => (naming = true)}>keep this view…</button>
      {/if}
    </div>

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
    {:else if shown.length === 0}
      <p class="apparatus-text note">Nothing written about them yet.</p>
    {:else}
      <ul class="list">
        {#each shown as row (row.id)}
          <li>
            <button
              type="button"
              class="row"
              onclick={() => jump(row)}
              disabled={isReading(row) && (isBusy() || whyCannotOpenAnother() !== null)}
              title={isReading(row)
                ? (whyCannotOpenAnother() ?? "open this reading")
                : row.anchor.kind === "chart"
                  ? "put the calculator back on this sky"
                  : row.anchor.kind === "person"
                    ? "everything about them"
                    : "everything filed under these"}
            >
              <span class="body">
                <span class="text">{row.text}</span>
                <span class="apparatus-text sub">
                  <span class="mark astro" title={rowMark(row).title}>{rowMark(row).glyph}</span>
                  {rowSubject(row)}
                </span>
              </span>
              <span class="meta apparatus-text">{dayLabel(row.writtenAt)}</span>
            </button>
            <!-- Outside the row button: a chip is its own control, and a
                 button inside a button is not markup a browser agrees on. -->
            {#if row.tags.length > 0}
              <span class="chips apparatus-text">
                {#each row.tags as tag, i}
                  <button
                    type="button"
                    class="chip"
                    title={`everything filed under ${row.tagLabels[i]}`}
                    onclick={() => byTag(tag)}>{row.tagLabels[i]}</button
                  >
                {/each}
              </span>
            {/if}
            <button
              type="button"
              class="remove"
              title="remove this note"
              aria-label="remove this note"
              onclick={(ev) => remove(ev, row, row.id)}
            >
              ✕
            </button>
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
  /* Writing a note that is about no sky. The plate below the words is how
     you say what it is about — the same grammar the reading view uses,
     minus the wheel. */
  .composing {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    border: 1px solid var(--line);
    border-top: 2px solid var(--brass);
    padding: 0.8rem;
    margin: 0.4rem 0.2rem 0.2rem;
  }
  .composing textarea {
    width: 100%;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--hairline);
    font: inherit;
    line-height: 1.6;
    padding: 0.2rem;
    resize: vertical;
  }
  .compose-foot {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
  }
  .grow {
    flex: 1;
  }
  /* The engraved mark that tells the three kinds apart. A mark, not a
     colour — the palette is CVD-validated and colour is never the only
     signal. */
  .mark {
    color: var(--ink-3);
    margin-right: 0.35rem;
  }
  /* Chips sit under the row rather than inside its button: a chip is its
     own control, and clicking one asks a different question from clicking
     the row. */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    grid-column: 1;
    padding: 0 0 0.3rem 0.2rem;
  }
  .chips .chip {
    background: none;
    border: 1px solid var(--line);
    border-radius: 2px;
    color: var(--ink-3);
    font: inherit;
    font-size: 0.74rem;
    padding: 0.02rem 0.32rem;
    cursor: pointer;
  }
  .chips .chip:hover {
    color: var(--ink);
    border-color: var(--hairline);
  }
  .narrowed {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3rem;
  }
  .chip-static {
    border: 1px solid var(--line);
    border-radius: 2px;
    padding: 0.02rem 0.32rem;
    font-size: 0.74rem;
  }

  /* The standing question: who it is about, and the views kept of it. Sits
     above `find`, which narrows whatever this leaves. */
  .asking {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 0.5rem 0.9rem;
    padding: 0.4rem 0.2rem 0;
  }
  .who-filter {
    display: flex;
    align-items: baseline;
    gap: 0.45rem;
  }
  .who-filter select {
    padding: 0.15rem 0.3rem;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--hairline);
    font: inherit;
  }
  .views {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .view-chip {
    display: inline-flex;
    align-items: stretch;
    border: 1px solid var(--line);
    border-radius: 2px;
  }
  .chip-open,
  .chip-drop {
    background: none;
    border: none;
    color: var(--ink-2);
    font: inherit;
    font-size: 0.82rem;
    padding: 0.1rem 0.45rem;
    cursor: pointer;
  }
  .chip-open:hover,
  .chip-drop:hover {
    color: var(--ink);
  }
  .chip-drop {
    border-left: 1px solid var(--line);
    color: var(--ink-3);
  }
  .naming {
    display: inline-flex;
    align-items: baseline;
    gap: 0.4rem;
  }
  .naming input {
    padding: 0.15rem 0.3rem;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--hairline);
    font: inherit;
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
  /* A grid rather than a row, so the chips can sit under the note they
     belong to instead of competing with it for the same line. */
  .list li {
    display: grid;
    grid-template-columns: 1fr auto;
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
