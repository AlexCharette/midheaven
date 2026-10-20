<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { deleteChart, getPreferences, listCharts } from "$lib/api";
  import { whyCannotOpenAnother } from "$lib/session.svelte";
  import { isBusy } from "$lib/busy.svelte";
  import { reopenReading } from "$lib/reopen";
  import { plural, t } from "$lib/chrome.svelte";
  import { notify } from "$lib/toasts.svelte";
  import type { ChartRow } from "$lib/types";
  import { reason } from "$lib/failure";
  import { chooseReadingsDir } from "$lib/readings";

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
  // shown.
  async function openReading(id: string) {
    err = "";
    try {
      await reopenReading(id);
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
  {#if loading}
    <p class="note">Reading the library…</p>
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
      <button class="btn primary small" onclick={choose}>Choose a folder…</button>
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
              <span class="name">{e.person?.name || "Untitled"}</span>
              <span class="sub"
                >{e.person?.born ?? ""}{e.person?.born && e.chart.place ? " · " : ""}{e.chart
                  .place}</span
              >
            </span>
            <span class="meta">{plural(t().notes, e.notes)}</span>
          </button>
          <button
            type="button"
            class="x-btn"
            title="remove from library"
            aria-label={`remove ${e.person?.name || "this"}'s reading`}
            onclick={(ev) => remove(ev, e)}
          >
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
              ><path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.6 4.5l.6 8.5h5.6l.6-8.5" /></svg
            >
          </button>
        </li>
      {/each}
    </ul>
  {/if}

</div>

<style>
  .note {
    margin: 2.4rem 0;
    text-align: center;
    font-family: var(--font-sans);
    font-size: 0.86rem;
    color: var(--ink-3);
  }
  .err-mark {
    color: var(--oxblood);
  }
  .list {
    list-style: none;
    margin: 0 -0.6rem;
    padding: 0;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    border-radius: var(--radius);
    transition: background var(--dur-fast) var(--ease-out-quint);
  }
  .list li:hover {
    background: var(--surface-hover);
  }
  .row {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    min-width: 0;
    padding: 0.7rem 0.4rem 0.7rem 0.6rem;
    text-align: left;
  }
  .row:disabled {
    cursor: wait;
  }
  .row:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: -2px;
    border-radius: var(--radius);
  }
  .who {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }
  /* the person is the one line set in the reading's own voice */
  .name {
    font-family: var(--font-serif);
    font-size: 1.02rem;
    color: var(--ink);
  }
  .sub {
    font-family: var(--font-sans);
    font-size: 0.8rem;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    flex: none;
    font-family: var(--font-sans);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }
  .x-btn {
    margin-right: 0.4rem;
  }
  @media (prefers-reduced-motion: reduce) {
    .list li {
      transition: none;
    }
  }
</style>
