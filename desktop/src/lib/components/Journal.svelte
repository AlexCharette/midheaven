<script lang="ts">
  // Every note the app holds, laid out on the plate they are about.
  //
  // Spoken and written alike, because they are the same thing — a view that
  // showed only the typed ones would revive a distinction the app does not
  // otherwise make.
  //
  // The journal used to be a list in a sheet over the calculator. It is a
  // screen now, because the question it answers — "what have I said about
  // Saturn?" — is asked by pointing at Saturn, and pointing wants the wheel.
  // With everyone in view the plate is the vocabulary itself (`VocabWheel`):
  // no sky, since the notes come from every sky. Narrowed to one person it is
  // their natal chart, drawn through `preview` so looking at it never makes it
  // the open reading. Pin an element and the plate zooms onto it, the newest
  // notes about it floating beside it; "see all" hands over to the list
  // column, which is also the whole journal whenever nothing is pinned.
  //
  // A row used to carry a `source` saying which of two stores it came from.
  // There is one store now: a row is a note, and the chart it names is either
  // someone's or a moment nobody was consulting over.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import ElementPlate from "./ElementPlate.svelte";
  import TopBar from "./TopBar.svelte";
  import VocabWheel from "./VocabWheel.svelte";
  import Wheel from "./Wheel.svelte";
  import {
    deleteNote,
    deleteView,
    getPreferences,
    listCharts,
    listElements,
    listPeople,
    listViews,
    preview,
    queryNotes,
    writeStandingNote,
    writeView,
  } from "$lib/api";
  import { isBusy } from "$lib/busy.svelte";
  import { whyCannotOpenAnother } from "$lib/session.svelte";
  import { natalOf, previewOf, rowMark, rowSubject, search, zoomOnto } from "$lib/journal";
  import { matching } from "$lib/focus";
  import { clearPins, mode, pinnedTags, resetFocus, setMode, toggle } from "$lib/focus.svelte";
  import { setMoment, setOptions, setPlace } from "$lib/preview.svelte";
  import { toMinutes } from "$lib/civil";
  import { notify } from "$lib/toasts.svelte";
  import { chooseReadingsDir } from "$lib/readings";
  import { reopenReading } from "$lib/reopen";
  import { prefersReducedMotion } from "$lib/motion";
  import type { Chart, ChartData, ChartRow, ElementDto, JournalRow, Person, View } from "$lib/types";
  import { SIDEREAL, TROPICAL, elementsOf } from "$lib/types";
  import { reason } from "$lib/failure";

  let { onclose }: { onclose: () => void } = $props();

  let rows = $state<JournalRow[]>([]);
  let query = $state("");
  let loading = $state(true);
  let err = $state("");
  let readingsDir = $state<string | null>(null);

  // Who the journal is narrowed to. The element half of the question is the
  // shared focus store's pins — the same pins the plate sets when clicked.
  let person = $state<string | undefined>(undefined);
  let people = $state<Person[]>([]);
  let charts = $state<ChartRow[]>([]);
  let views = $state<View[]>([]);
  let elements = $state<ElementDto[]>([]);
  let naming = $state(false);
  let viewName = $state("");

  // Their chart, when the journal is narrowed to someone who has one.
  let natal = $state<ChartData | null>(null);

  // Writing a standing note. Whether it is craft or an observation about
  // someone falls out of the person selector rather than being a third thing
  // to say: name nobody and it is about the elements alone.
  let composing = $state(false);
  let draft = $state("");
  let draftTags = new SvelteSet<string>();
  let draftPerson = $state("");
  let saving = $state(false);

  // "See all": the list column taking over from the floating notes while an
  // element is pinned. With nothing pinned the column simply is the journal.
  let listing = $state(false);

  const pins = $derived(pinnedTags());
  const found = $derived(search(rows, query));
  const matched = $derived(matching(found, pins, mode()) as JournalRow[]);
  const FLOAT = 5;
  const floating = $derived(matched.slice(0, FLOAT));
  // Below the split width nothing floats, so the list stays the answer.
  let wide = $state(true);
  const columnOpen = $derived(!wide || pins.length === 0 || listing || composing);

  // The plate's density: how much has been written under each tag, within
  // whoever the journal is narrowed to.
  const counts = $derived.by(() => {
    const m = new Map<string, number>();
    for (const r of rows) for (const t of r.tags) m.set(t, (m.get(t) ?? 0) + 1);
    return m;
  });

  // A tag's name, from the vocabulary or — for aspect pairs, which only a
  // chart has — from the natal chart on the plate.
  const names = $derived.by(() => {
    const m = new Map(elements.map((e) => [e.tag, e.name]));
    if (natal) for (const e of elementsOf(natal)) m.set(e.tag, e.name);
    return m;
  });
  const nameOf = (tag: string) => names.get(tag) ?? tag;
  const personName = $derived(people.find((p) => p.id === person)?.name ?? "");

  onMount(() => {
    // The plate's pins are the shared focus; a calculator note or a reading
    // must not leave theirs lit here, nor this leave its own behind.
    resetFocus();
    listElements()
      .then((e) => (elements = e))
      .catch(() => {});
    refresh();
    return resetFocus;
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
      [rows, people, views, charts] = await Promise.all([
        queryNotes({ tags: [], mode: "any", person }),
        listPeople(),
        listViews(),
        listCharts(),
      ]);
    } catch (e) {
      err = reason(e);
    } finally {
      loading = false;
    }
  }

  // The plate follows the person: their chart when the store has one, the
  // vocabulary otherwise. A slow answer for someone no longer chosen is
  // dropped rather than drawn.
  let asked = 0;
  async function drawFor(id: string | undefined) {
    const seq = ++asked;
    const chart: Chart | null = id ? natalOf(id, charts) : null;
    if (!chart) {
      natal = null;
      return;
    }
    try {
      const { chart: sky } = await preview(previewOf(chart));
      if (seq === asked) natal = sky;
    } catch {
      if (seq === asked) natal = null;
    }
  }

  /** Narrow to one person, or to everyone. */
  async function byPerson(id: string) {
    person = id || undefined;
    clearPins();
    listing = false;
    await refresh();
    await drawFor(person);
  }

  /** Pin exactly these tags — what a chip, a kept view or a standing note's
   * row asks for. */
  function pinOnly(tags: string[]) {
    clearPins();
    for (const t of tags) toggle(t);
    listing = false;
  }

  /** Keep the current question under a name. */
  async function keepView() {
    const name = viewName.trim();
    if (!name) return;
    try {
      await writeView(name, { tags: pins, mode: mode(), person });
      viewName = "";
      naming = false;
      views = await listViews();
      notify(`kept “${name}”`);
    } catch (e) {
      err = reason(e);
    }
  }

  async function applyView(v: View) {
    if ((v.query.person ?? undefined) !== person) await byPerson(v.query.person ?? "");
    setMode(v.query.mode);
    pinOnly(v.query.tags);
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

  /** Open the composer, filed in advance under `tags` — what writing from an
   * element's empty card asks for: a note about the thing being looked at. */
  function beginCompose(tags: string[] = []) {
    composing = true;
    draft = "";
    draftTags.clear();
    for (const t of tags) draftTags.add(t);
    draftPerson = person ?? "";
  }

  async function keepNote() {
    const words = draft.trim();
    if (!words) {
      err = "A note needs words.";
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

  async function choose() {
    if (await chooseReadingsDir()) refresh();
  }

  /** Put the calculator back on the sky a note was written over. The seed is
   * all it takes — the chart was never stored, only what recomputes it. */
  function restoreIts(chart: Chart) {
    const { seed, houseSystem, ayanamsa, locale, place } = chart;
    const minutes = toMinutes(seed.date, seed.time);
    if (minutes === null) {
      err = "That note's moment cannot be read.";
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
  const isReading = (row: JournalRow) => row.anchor.kind === "chart" && row.person !== undefined;

  /** What clicking a note does, which is different for each of the three
   * kinds — and in every case is "take me to the rest of this".
   *
   * A reading note reopens the reading. A note over a moment nobody was
   * consulting puts the calculator back on that sky. A note about a person
   * narrows the journal to them, and a standing note pins what it is filed
   * under — the only "more of this" either has. */
  async function jump(row: JournalRow) {
    switch (row.anchor.kind) {
      case "chart":
        if (row.chart === undefined) return;
        if (isReading(row)) {
          try {
            await reopenReading(row.chart.id);
          } catch (e) {
            err = reason(e);
          }
        } else restoreIts(row.chart);
        return;
      case "person":
        await byPerson(row.anchor.id);
        return;
      case "standing":
        pinOnly(row.tags);
    }
  }

  async function remove(ev: MouseEvent, row: JournalRow) {
    ev.stopPropagation();
    const sure = await ask(`Remove this note?\n\n${row.text}\n\nIt is not recoverable.`, {
      title: "Midheaven",
      kind: "warning",
    });
    if (!sure) return;
    try {
      await deleteNote(row.id);
      rows = rows.filter((r) => r.id !== row.id);
      notify("note removed");
    } catch (e) {
      err = reason(e);
    }
  }

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
  const rowTitle = (row: JournalRow) =>
    isReading(row)
      ? (whyCannotOpenAnother() ?? "open this reading")
      : row.anchor.kind === "chart"
        ? "put the calculator back on this sky"
        : row.anchor.kind === "person"
          ? "everything about them"
          : "everything filed under these";

  // ---- the zoom ----
  //
  // The plate scales about its own top-left; `zoomOnto` says how far to carry
  // it so the pinned element rests off-centre with open sky on its outward
  // side, and that is where its notes float. Measured from the element's own
  // box on whichever plate is drawn — both tag their elements `data-tag`.
  let stage = $state<HTMLElement | null>(null);
  let plate = $state<HTMLElement | null>(null);
  let cardsEl = $state<HTMLElement | null>(null);
  let zoom = $state<{ tx: number; ty: number; s: number } | null>(null);
  let rest = $state<{ x: number; y: number } | null>(null);
  let side = $state<"left" | "right">("right");
  let leaders = $state<{ x: number; y: number }[]>([]);
  // how far from the element's centre its leaders start: clear of the glyph
  let clear = $state(0);

  const SCALE = 1.85;

  function aim() {
    const tag = pins.at(-1);
    if (!tag || !stage || !plate || !wide) {
      zoom = null;
      rest = null;
      return;
    }
    const node = plate.querySelector(`[data-tag="${CSS.escape(tag)}"]`) as SVGGraphicsElement | null;
    const svg = node?.ownerSVGElement;
    if (!node || !svg) {
      zoom = null;
      rest = null;
      return;
    }
    const bb = node.getBBox();
    const vb = svg.viewBox.baseVal;
    const k = svg.clientWidth / vb.width;
    const toStage = (ux: number, uy: number) => ({
      x: plate!.offsetLeft + (ux - vb.x) * k,
      y: plate!.offsetTop + (uy - vb.y) * k,
    });
    const at = toStage(bb.x + bb.width / 2, bb.y + bb.height / 2);
    const centre = toStage(360, 360);
    // positions inside the plate box, which is what the transform scales
    const local = { x: at.x - plate.offsetLeft, y: at.y - plate.offsetTop };
    const z = zoomOnto(local, { x: centre.x - plate.offsetLeft, y: centre.y - plate.offsetTop }, { w: stage.clientWidth, h: stage.clientHeight }, SCALE, 100 * k);
    zoom = { tx: z.tx - plate.offsetLeft, ty: z.ty - plate.offsetTop, s: SCALE };
    clear = (Math.min(Math.max(bb.width, bb.height), 60) / 2 + 6) * k * SCALE;
    rest = z.rest;
    side = z.side;
  }

  // Re-aim whenever what is pinned, what is drawn, or the room changes.
  $effect(() => {
    void pins;
    void natal;
    void columnOpen;
    void elements;
    tick().then(aim);
  });
  onMount(() => {
    const media = window.matchMedia("(min-width: 960px)");
    const onMedia = () => {
      wide = media.matches;
      aim();
    };
    onMedia();
    media.addEventListener("change", onMedia);
    const ro = new ResizeObserver(() => aim());
    if (stage) ro.observe(stage);
    return () => {
      media.removeEventListener("change", onMedia);
      ro.disconnect();
    };
  });

  // The hairlines from the element to each floating note, measured once the
  // cards have laid themselves out.
  $effect(() => {
    void floating;
    void rest;
    void side;
    tick().then(() => {
      if (!cardsEl || !stage || !rest) return (leaders = []);
      const s = stage.getBoundingClientRect();
      leaders = [...cardsEl.querySelectorAll<HTMLElement>(".card")].map((c) => {
        const r = c.getBoundingClientRect();
        return { x: (side === "right" ? r.left : r.right) - s.left, y: r.top + r.height / 2 - s.top };
      });
    });
  });

  function onkey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (composing) composing = false;
    else if (listing && pins.length) listing = false;
    else if (pins.length) clearPins();
    else onclose();
  }

  const reduced = prefersReducedMotion();
</script>

<svelte:window onkeydown={onkey} />

<TopBar>
  {#snippet lead()}
    <button type="button" class="btn quiet icon" onclick={onclose} title="back to the sky" aria-label="back">
      <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M10 3.5 5.5 8l4.5 4.5" /></svg>
    </button>
    <div class="title">
      <h1>Journal</h1>
      <p class="sub">{personName ? `About ${personName}` : "Everything written down"} · {rows.length} {rows.length === 1 ? "note" : "notes"}</p>
    </div>
  {/snippet}
  {#snippet acts()}
    <label class="find">
      <svg class="find-icon" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
        ><circle cx="7" cy="7" r="4.6" /><path d="m10.4 10.4 3.4 3.4" /></svg
      >
      <input type="search" bind:value={query} placeholder="Find a word, a person" aria-label="search notes" />
    </label>
    <label class="about">
      <span class="sr-only">About</span>
      <select value={person ?? ""} onchange={(e) => byPerson(e.currentTarget.value)} aria-label="narrow to one person">
        <option value="">Everyone</option>
        {#each people as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </label>
    <button type="button" class="btn quiet" onclick={() => beginCompose()} disabled={composing}>
      <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
        ><path d="M3 13h2.4L12.6 5.8a1.7 1.7 0 0 0-2.4-2.4L3 10.6V13Z" /><path d="m9.3 4.3 2.4 2.4" /></svg
      >
      Write a note
    </button>
  {/snippet}
</TopBar>

<div class="journal" class:with-column={columnOpen}>
  <section class="stage" bind:this={stage} aria-label="the plate">
    <div
      class="plate"
      class:zoomed={zoom !== null}
      class:still={reduced}
      bind:this={plate}
      style:transform={zoom ? `translate(${zoom.tx}px, ${zoom.ty}px) scale(${zoom.s})` : undefined}
    >
      {#if natal}
        <Wheel chart={natal} notes={rows} />
      {:else}
        <VocabWheel {elements} {counts} />
      {/if}
    </div>

    {#if pins.length === 0}
      <p class="caption">
        {#if natal}
          {personName}'s natal chart. Pick an element to read what has been said about it.
        {:else if person}
          No chart of {personName}'s is on file, so this is the vocabulary. Pick an element to read what has been said about it.
        {:else}
          The vocabulary — every chart at once. Pick an element to read what has been said about it.
        {/if}
      </p>
    {:else}
      <!-- what is pinned, as a line you can undo -->
      <div class="selection">
        {#each pins as tag (tag)}
          <button type="button" class="pin" onclick={() => toggle(tag)} aria-label={`unpin ${nameOf(tag)}`}>
            {nameOf(tag)}
            <svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" /></svg>
          </button>
        {/each}
        {#if pins.length > 1}
          <span class="seg" role="group" aria-label="match">
            <button type="button" aria-pressed={mode() === "any"} onclick={() => setMode("any")}>any</button
            ><button type="button" aria-pressed={mode() === "all"} onclick={() => setMode("all")}>all</button>
          </span>
        {/if}
        <span class="sel-count">{matched.length} {matched.length === 1 ? "note" : "notes"}</span>
      </div>
    {/if}

    <!-- The notes, floating beside the element they are about. -->
    {#if rest && !columnOpen}
      <svg class="leaders" aria-hidden="true">
        {#each leaders as l, i (i)}
          {@const a = Math.atan2(l.y - rest.y, l.x - rest.x)}
          <line
            x1={rest.x + Math.cos(a) * clear}
            y1={rest.y + Math.sin(a) * clear}
            x2={l.x}
            y2={l.y}
            style:--i={i}
          />
        {/each}
      </svg>
      <div
        class="cards {side}"
        bind:this={cardsEl}
        style:left={side === "right" ? `${rest.x + 64}px` : undefined}
        style:right={side === "left" && stage ? `${stage.clientWidth - rest.x + 64}px` : undefined}
        style:top={`${rest.y}px`}
      >
        {#each floating as row, i (row.id)}
          <button type="button" class="card" style:--i={i} onclick={() => jump(row)} title={rowTitle(row)}>
            <span class="card-text">{row.text}</span>
            <span class="card-meta">
              <span class="astro" title={rowMark(row).title}>{rowMark(row).glyph}</span>
              {rowSubject(row) || "Standing note"} · {dayLabel(row.writtenAt)}
            </span>
          </button>
        {:else}
          <div class="card none">
            <p>Nothing written about {pins.map(nameOf).join(" and ")} yet.</p>
            <button type="button" class="btn primary small" onclick={() => beginCompose(pins)}>
              <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
                ><path d="M3 13h2.4L12.6 5.8a1.7 1.7 0 0 0-2.4-2.4L3 10.6V13Z" /><path d="m9.3 4.3 2.4 2.4" /></svg
              >
              Write a note on {pins.length === 1 ? nameOf(pins[0]) : "these"}
            </button>
          </div>
        {/each}
        {#if matched.length > FLOAT}
          <button type="button" class="see-all" style:--i={FLOAT} onclick={() => (listing = true)}>
            See all {matched.length}
            <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M6 3.5 10.5 8 6 12.5" /></svg>
          </button>
        {/if}
      </div>
    {/if}
  </section>

  <aside class="column" aria-label="notes" inert={!columnOpen}>
    <div class="column-inner form">
      {#if loading}
        <p class="note">Reading the journal…</p>
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
          <button class="btn primary small" onclick={choose}>Choose a folder…</button>
        </div>
      {:else}
        {#if composing}
          <div class="composing">
            <h2 class="section-head">A standing note</h2>
            <!-- svelte-ignore a11y_autofocus -->
            <textarea
              bind:value={draft}
              rows="3"
              autofocus
              placeholder={draftTags.size === 1
                ? `What you know about ${nameOf([...draftTags][0])}`
                : "What you know — about Saturn, about Leo, about someone"}
              aria-label="the note"
            ></textarea>
            <ElementPlate
              selected={draftTags}
              ontoggle={(tag) => (draftTags.has(tag) ? draftTags.delete(tag) : draftTags.add(tag))}
            />
            <!-- said in words too: an aspect pair carried over from a natal
                 plate has no entry on the vocabulary plate above to light -->
            <p class="filed">
              {draftTags.size ? `Filed under ${[...draftTags].map(nameOf).join(", ")}` : "Filed under nothing yet"}
            </p>
            <div class="compose-foot">
              <select bind:value={draftPerson} aria-label="who this note is about">
                <option value="">About no one — the elements</option>
                {#each people as p (p.id)}
                  <option value={p.id}>About {p.name}</option>
                {/each}
              </select>
              <span class="grow"></span>
              <button type="button" class="btn bare small" onclick={() => (composing = false)}>Cancel</button>
              <button type="button" class="btn primary small" disabled={saving} onclick={keepNote}>
                {saving ? "Keeping…" : "Keep"}
              </button>
            </div>
          </div>
        {/if}

        <div class="column-head">
          <h2 class="section-head">
            {pins.length ? pins.map(nameOf).join(mode() === "all" ? " + " : " · ") : "All notes"}
            <span class="n">{matched.length}</span>
          </h2>
          {#if listing && pins.length}
            <button type="button" class="btn bare small" onclick={() => (listing = false)}>Back to the plate</button>
          {/if}
        </div>

        {#if views.length > 0 || pins.length > 0 || person}
          <div class="views">
            {#each views as v (v.id)}
              <span class="pill view">
                <button type="button" onclick={() => applyView(v)} title="ask this again">{v.name}</button>
                <button type="button" class="drop" onclick={(ev) => dropView(ev, v)} title="forget this view" aria-label={`forget ${v.name}`}>
                  <svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" /></svg>
                </button>
              </span>
            {/each}
            {#if naming}
              <span class="naming">
                <input
                  type="text"
                  bind:value={viewName}
                  placeholder="Call it something"
                  aria-label="name for this view"
                  onkeydown={(e) => {
                    if (e.key === "Enter") keepView();
                    if (e.key === "Escape") {
                      naming = false;
                      viewName = "";
                    }
                  }}
                />
                <button type="button" class="btn quiet small" onclick={keepView}>Keep</button>
              </span>
            {:else if pins.length > 0 || person}
              <button type="button" class="btn bare small" onclick={() => (naming = true)}>Keep this view…</button>
            {/if}
          </div>
        {/if}

        {#if rows.length === 0}
          <div class="empty-plate">
            <span class="mark" aria-hidden="true">✶</span>
            <p class="caption">Nothing written down yet.</p>
            <p class="sub">Notes you write, and the words of readings you transcribe, appear here.</p>
          </div>
        {:else if matched.length === 0}
          <p class="note">{query ? `Nothing here matches “${query}”.` : "Nothing is filed under that yet."}</p>
        {:else}
          <ul class="list">
            {#each matched as row (row.id)}
              <li>
                <button
                  type="button"
                  class="jrow"
                  onclick={() => jump(row)}
                  disabled={isReading(row) && (isBusy() || whyCannotOpenAnother() !== null)}
                  title={rowTitle(row)}
                >
                  <span class="text">{row.text}</span>
                  <span class="sub">
                    <span class="mark astro" title={rowMark(row).title}>{rowMark(row).glyph}</span>
                    {rowSubject(row) || "Standing note"} · {dayLabel(row.writtenAt)}
                  </span>
                </button>
                <!-- Outside the row button: a chip is its own control, and a
                     button inside a button is not markup a browser agrees on. -->
                {#if row.tags.length > 0}
                  <span class="chips">
                    {#each row.tags as tag, i (tag)}
                      <button
                        type="button"
                        class="pill"
                        class:on={pins.includes(tag)}
                        title={`everything filed under ${row.tagLabels[i]}`}
                        onclick={() => pinOnly([tag])}>{row.tagLabels[i]}</button
                      >
                    {/each}
                  </span>
                {/if}
                <button type="button" class="x-btn" title="remove this note" aria-label="remove this note" onclick={(ev) => remove(ev, row)}>
                  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
                    ><path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.6 4.5l.6 8.5h5.6l.6-8.5" /></svg
                  >
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </div>
  </aside>
</div>

<style>
  /* --- the bar --- */
  .title {
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 1.15rem;
    font-weight: 400;
    line-height: 1.2;
    color: var(--ink);
  }
  .title .sub {
    margin: 0.1rem 0 0;
    font-size: 0.78rem;
    color: var(--ink-3);
    white-space: nowrap;
  }
  .find {
    position: relative;
    display: flex;
    align-items: center;
  }
  .find-icon {
    position: absolute;
    left: 0.65rem;
    fill: none;
    stroke: var(--ink-3);
    stroke-width: 1.3;
    stroke-linecap: round;
    pointer-events: none;
  }
  .find input,
  .about select {
    height: 2.15rem;
    font-family: var(--font-sans);
    font-size: 0.84rem;
    color: var(--ink);
    background-color: var(--surface);
    border: none;
    border-radius: var(--radius-sm);
    box-shadow: inset 0 0 0 1px var(--line);
    transition: box-shadow var(--dur-fast) var(--ease-out-quint);
  }
  .find input {
    width: clamp(10rem, 18vw, 16rem);
    padding: 0 0.7rem 0 2rem;
  }
  .find input::placeholder {
    font-style: normal;
    color: var(--ink-3);
  }
  .about select {
    appearance: none;
    -webkit-appearance: none;
    padding: 0 1.9rem 0 0.7rem;
    cursor: pointer;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath d='M1 1l4 3.8L9 1' fill='none' stroke='%239a957f' stroke-width='1.3' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.7rem center;
  }
  .find input:focus,
  .about select:focus {
    outline: none;
    box-shadow:
      inset 0 0 0 1px var(--brass),
      0 0 0 3px var(--brass-wash);
  }
  .about option {
    background: var(--bg-deep);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* --- the screen: the plate's stage, and the list column beside it --- */
  .journal {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 0;
    height: calc(100vh - var(--bar-h));
    transition: grid-template-columns var(--dur-slow) var(--ease-out-expo);
  }
  .journal.with-column {
    grid-template-columns: minmax(0, 1fr) min(26rem, 38vw);
  }
  .stage {
    position: relative;
    overflow: hidden;
    display: grid;
    place-items: center;
  }
  .plate {
    width: min(calc(100vh - var(--bar-h) - 7rem), 92%, 760px);
    transform-origin: 0 0;
    transition:
      transform var(--dur-slow) var(--ease-out-expo),
      opacity var(--dur-slow) var(--ease-out-expo);
  }
  .plate.still {
    transition: none;
  }
  /* zoomed, the plate becomes the ground the notes stand on */
  .plate.zoomed {
    opacity: 0.85;
  }
  .caption {
    position: absolute;
    left: 50%;
    bottom: 1.4rem;
    transform: translateX(-50%);
    width: min(36rem, 90%);
    margin: 0;
    text-align: center;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    color: var(--ink-3);
    text-wrap: balance;
  }
  .selection {
    position: absolute;
    top: 1rem;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.3rem;
    border-radius: 999px;
    background: rgba(15, 18, 43, 0.82);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 8px 24px -10px rgba(0, 0, 0, 0.6);
    font-family: var(--font-sans);
    z-index: 3;
  }
  .pin {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    height: 1.8rem;
    padding: 0 0.6rem 0 0.75rem;
    border-radius: 999px;
    background: var(--brass-wash);
    box-shadow: inset 0 0 0 1px var(--brass-halo);
    font-size: 0.8rem;
    color: var(--ink);
  }
  .pin svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    opacity: 0.7;
  }
  .pin:hover svg {
    opacity: 1;
  }
  .seg {
    display: inline-flex;
    padding: 2px;
    border-radius: 999px;
    background: var(--surface);
  }
  .seg button {
    padding: 0.15rem 0.6rem;
    border-radius: 999px;
    font-size: 0.75rem;
    color: var(--ink-3);
  }
  .seg button[aria-pressed="true"] {
    background: var(--surface-raised);
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  .sel-count {
    padding: 0 0.6rem 0 0.3rem;
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }

  /* --- the floating notes --- */
  .leaders {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
  }
  .leaders line {
    stroke: var(--brass-halo);
    stroke-width: 1;
    animation: draw var(--dur-slow) var(--ease-out-expo) both;
    animation-delay: calc(240ms + var(--i) * 50ms);
  }
  @keyframes draw {
    from {
      opacity: 0;
    }
  }
  .cards {
    position: absolute;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    width: min(21rem, 34vw);
    transform: translateY(-50%);
  }
  .cards.left {
    align-items: flex-end;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    width: 100%;
    padding: 0.8rem 0.95rem 0.75rem;
    border-radius: var(--radius);
    background: rgba(26, 31, 69, 0.94);
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 14px 36px -14px rgba(0, 0, 0, 0.75);
    text-align: left;
    animation: float-in var(--dur-slow) var(--ease-out-expo) both;
    animation-delay: calc(200ms + var(--i) * 60ms);
    transition:
      box-shadow var(--dur-fast) var(--ease-out-quint),
      transform var(--dur-fast) var(--ease-out-quint);
  }
  .card:hover {
    transform: translateX(var(--nudge, 2px));
    box-shadow:
      inset 0 0 0 1px var(--hairline),
      0 18px 40px -14px rgba(0, 0, 0, 0.8);
  }
  .cards.left .card {
    --nudge: -2px;
  }
  .card:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 2px;
  }
  .card-text {
    font-family: var(--font-serif);
    font-size: 0.95rem;
    line-height: 1.5;
    color: var(--ink);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
  }
  .card-meta {
    font-family: var(--font-sans);
    font-size: 0.74rem;
    color: var(--ink-3);
  }
  .card.none {
    gap: 0.7rem;
    align-items: flex-start;
    font-family: var(--font-sans);
    font-size: 0.82rem;
    color: var(--ink-3);
  }
  .card.none p {
    margin: 0;
  }
  .card.none:hover {
    transform: none;
  }
  .filed {
    margin: 0;
    font-size: 0.78rem;
    color: var(--ink-3);
  }
  .see-all {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.3rem 0.2rem;
    font-family: var(--font-sans);
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--brass);
    animation: float-in var(--dur-slow) var(--ease-out-expo) both;
    animation-delay: calc(200ms + var(--i) * 60ms);
  }
  .see-all svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  @keyframes float-in {
    from {
      opacity: 0;
      transform: translateX(calc(var(--from, -10px)));
    }
  }
  .cards.left .card,
  .cards.left .see-all {
    --from: 10px;
  }

  /* --- the list column --- */
  .column {
    min-width: 0;
    overflow: hidden;
    box-shadow: inset 1px 0 0 var(--line);
  }
  .column-inner {
    width: min(26rem, 38vw);
    height: 100%;
    overflow-y: auto;
    padding: 0.4rem 1.4rem 2rem;
    gap: 0.8rem;
  }
  .column-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
  }
  .column-head .section-head {
    margin: 0;
    color: var(--ink-2);
  }
  .column-head .n {
    margin-left: 0.35rem;
    font-weight: 400;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .note {
    margin: 2rem 0;
    text-align: center;
    font-size: 0.84rem;
    color: var(--ink-3);
  }
  .err-mark {
    color: var(--oxblood);
  }
  .composing {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--line);
  }
  .composing .section-head {
    margin: 0;
  }
  .compose-foot {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .compose-foot select {
    width: auto;
    min-height: 1.95rem;
    padding-top: 0.2rem;
    padding-bottom: 0.2rem;
    font-size: 0.8rem;
  }
  .grow {
    flex: 1;
  }
  .views {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
  }
  .naming {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .column .naming input {
    width: 11rem;
    min-height: 1.95rem;
    padding-top: 0.2rem;
    padding-bottom: 0.2rem;
    font-size: 0.8rem;
  }
  /* A pill is a question you can ask again: a kept view, an element a note
     is filed under. Tonal, never framed. */
  .pill {
    display: inline-flex;
    align-items: center;
    height: 1.65rem;
    padding: 0 0.6rem;
    border-radius: 999px;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 0.74rem;
    color: var(--ink-2);
    white-space: nowrap;
    transition:
      background var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  button.pill:hover,
  .pill.view:hover {
    background: var(--surface-hover);
    color: var(--ink);
  }
  .pill.on {
    background: var(--brass-wash);
    box-shadow: inset 0 0 0 1px var(--brass-halo);
    color: var(--ink);
  }
  .pill.view {
    padding-right: 0.25rem;
    gap: 0.2rem;
  }
  .pill .drop {
    display: grid;
    place-items: center;
    width: 1.2rem;
    height: 1.2rem;
    border-radius: 50%;
    color: var(--ink-3);
  }
  .pill .drop svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
  }
  .pill .drop:hover {
    color: var(--ink);
    background: var(--surface-hover);
  }
  .list {
    list-style: none;
    margin: 0 -0.6rem;
    padding: 0;
  }
  /* A grid rather than a row, so the chips can sit under the note they
     belong to instead of competing with it for the same line. */
  .list li {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: start;
    border-radius: var(--radius);
    transition: background var(--dur-fast) var(--ease-out-quint);
  }
  .list li + li {
    margin-top: 0.1rem;
  }
  .list li:hover {
    background: var(--surface-hover);
  }
  .jrow {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
    padding: 0.65rem 0.4rem 0.4rem 0.6rem;
    text-align: left;
  }
  .jrow:disabled {
    cursor: wait;
  }
  .jrow:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: -2px;
    border-radius: var(--radius);
  }
  .x-btn {
    grid-column: 2;
    grid-row: 1 / span 2;
    align-self: start;
    margin: 0.5rem 0.4rem 0 0;
  }
  /* two lines of the note, then an ellipsis — a row is an excerpt of itself;
     the words keep the reading's own voice */
  .text {
    font-family: var(--font-serif);
    font-size: 0.94rem;
    line-height: 1.5;
    color: var(--ink);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .sub {
    font-size: 0.75rem;
    color: var(--ink-3);
  }
  /* The engraved mark that tells the three kinds apart. A mark, not a
     colour — the palette is CVD-validated and colour is never the only
     signal. */
  .mark {
    margin-right: 0.15rem;
    color: var(--ink-3);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    grid-column: 1;
    padding: 0 0 0.65rem 0.6rem;
  }

  /* --- narrow: the plate leads, the list follows; nothing floats --- */
  @media (max-width: 640px) {
    .find {
      flex: 1 1 100%;
      min-width: 0;
    }
    .find input {
      width: 100%;
    }
  }
  @media (max-width: 959px) {
    .journal,
    .journal.with-column {
      display: block;
      height: auto;
    }
    .stage {
      height: auto;
      padding: 0.5rem 0 3.4rem;
    }
    .plate {
      width: min(92vw, 560px);
    }
    .column {
      box-shadow: inset 0 1px 0 var(--line);
    }
    .column-inner {
      width: auto;
      height: auto;
      overflow: visible;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .journal,
    .plate,
    .card,
    .see-all,
    .leaders line,
    .list li,
    .pill {
      transition: none;
      animation: none;
    }
  }
</style>
