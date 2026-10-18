<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { deleteChart, getPreferences, listCharts, openChart } from "$lib/api";
  import { openReading as installReading, whyCannotOpenAnother } from "$lib/session.svelte";
  import { during, isBusy } from "$lib/busy.svelte";
  import { plural, t } from "$lib/chrome.svelte";
  import { notify } from "$lib/toasts.svelte";
  import type { ChartRow } from "$lib/types";
  import { reason } from "$lib/failure";
  import { chooseReadingsDir } from "$lib/readings";
  import { refreshNotes } from "$lib/notes.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let entries = $state<ChartRow[]>([]);
  let loading = $state(true);
  let err = $state("");
  let readingsDir = $state<string | null>(null);

  // once on mount (no reactive reads in the body)
  $effect(() => {
    refresh();
  });

  async function refresh() {
    loading = true;
    err = "";
    try {
      readingsDir = (await getPreferences()).readings_dir;
      entries = await listCharts();
    } catch (e) {
      err = reason(e);
    } finally {
      loading = false;
    }
  }

  // Installing the reading makes +page.svelte swap to the reading view,
  // unmounting this panel; a failed load leaves the panel up with the error
  // shown. Refused mid-take: it would re-seat the chart under a live recorder,
  // and the take would then land on this reading instead of the one it was
  // spoken over.
  async function openReading(id: string) {
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

  // An explicit click is not a decline, so this always asks — `chooseReadingsDir`
  // rather than the rule-gated `ensureReadingsDir`. Refreshing afterwards is
  // what turns this empty plate into the list.
  async function choose() {
    if (await chooseReadingsDir()) refresh();
  }

  async function remove(ev: MouseEvent, e: ChartRow) {
    ev.stopPropagation();
    const who = e.person?.name || "this";
    const n = e.notes;
    const sure = await ask(
      `Remove ${who}'s reading?\n\n${n} ${n === 1 ? "note" : "notes"} written about it ` +
        `will go with it. It is not recoverable.`,
      { title: "Midheaven", kind: "warning" },
    );
    if (!sure) return;
    try {
      await deleteChart(e.chart.id);
      entries = entries.filter((x) => x.chart.id !== e.chart.id);
      notify("reading removed");
    } catch (e2) {
      err = String(e2);
    }
  }

</script>

<div class="library">
  <p class="rubric">a saved reading</p>

  {#if loading}
    <p class="apparatus-text note">reading the library…</p>
  {:else if err}
    <div class="empty-plate">
      <span class="mark err-mark" aria-hidden="true">✗</span>
      <p class="caption">The library could not be read.</p>
      <p class="sub">{err}</p>
    </div>
  {:else if !readingsDir}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">No readings folder is set.</p>
      <p class="sub">Choose one to keep a library of saved readings.</p>
      <button class="frame-btn" onclick={choose}>choose a folder…</button>
    </div>
  {:else if entries.length === 0}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">No readings saved here yet.</p>
      <p class="sub">Computed and transcribed readings you save will appear here.</p>
    </div>
  {:else}
    <ul class="list">
      {#each entries as e (e.chart.id)}
        <li>
          <button
            type="button"
            class="row"
            onclick={() => openReading(e.chart.id)}
            disabled={isBusy() || whyCannotOpenAnother() !== null}
            title={whyCannotOpenAnother() ?? undefined}
          >
            <span class="who">
              <span class="name">{e.person?.name || "untitled"}</span>
              <span class="apparatus-text sub"
                >{e.person?.born ?? ""}{e.person?.born && e.chart.place ? " · " : ""}{e.chart
                  .place}</span
              >
            </span>
            <span class="meta apparatus-text">{plural(t().notes, e.notes)}</span>
          </button>
          <button
            type="button"
            class="remove"
            title="remove from library"
            aria-label={`remove ${e.person?.name || "this"}'s reading`}
            onclick={(ev) => remove(ev, e)}
          >
            ✕
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="foot">
    <button type="button" class="ghost" onclick={onclose}>← back</button>
  </p>
</div>

<style>
  .library {
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
  /* a manicule slides in on hover, the same pointing hand the index uses */
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
  .row:hover:not(:disabled) .name {
    text-decoration: underline;
  }
  @media (prefers-reduced-motion: reduce) {
    .row,
    .row::before {
      transition: none;
    }
  }
  .who {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    min-width: 0;
  }
  .name {
    color: var(--ink);
    font-size: 1.05rem;
  }
  .sub {
    font-size: 0.85rem;
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
    text-align: center;
    margin-top: 1.8rem;
    font-size: 0.85rem;
  }
</style>
