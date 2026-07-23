<script lang="ts">
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { deleteReading, getPreferences, listReadings, loadChart } from "$lib/api";
  import { app, notify } from "$lib/state.svelte";
  import type { ReadingEntry } from "$lib/types";

  let { onclose }: { onclose: () => void } = $props();

  let entries = $state<ReadingEntry[]>([]);
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
      entries = await listReadings();
    } catch (e) {
      err = String(e);
    } finally {
      loading = false;
    }
  }

  // Setting app.chart makes +page.svelte swap to the reading view, unmounting
  // this panel; a failed load leaves the panel up with the error shown.
  async function openReading(chartPath: string) {
    if (app.busy !== false) return;
    err = "";
    app.busy = "compute";
    try {
      app.chart = await loadChart(chartPath);
      notify(`opened ${app.chart.meta.name}'s reading`);
    } catch (e) {
      err = String(e);
    } finally {
      app.busy = false;
    }
  }

  async function openFile() {
    const path = await open({
      multiple: false,
      directory: false,
      defaultPath: readingsDir ?? undefined,
      filters: [{ name: "Midheaven reading", extensions: ["json"] }],
    });
    if (typeof path === "string") openReading(path);
  }

  async function remove(ev: MouseEvent, e: ReadingEntry) {
    ev.stopPropagation();
    const who = e.name || "this";
    const sure = await ask(
      `Remove ${who}'s reading from the library?\n\n${e.dir}\n\nThe folder and its files are not recoverable.`,
      { title: "Midheaven", kind: "warning" },
    );
    if (!sure) return;
    try {
      await deleteReading(e.dir);
      entries = entries.filter((x) => x.dir !== e.dir);
      notify("reading removed");
    } catch (e2) {
      err = String(e2);
    }
  }

  const savedLabel = (ms: number) => {
    const d = new Date(ms);
    const sameYear = d.getFullYear() === new Date().getFullYear();
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      ...(sameYear ? {} : { year: "numeric" }),
    });
  };
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
      <p class="sub">Choose one in preferences to keep a library of saved readings.</p>
    </div>
  {:else if entries.length === 0}
    <div class="empty-plate">
      <span class="mark" aria-hidden="true">✶</span>
      <p class="caption">No readings saved here yet.</p>
      <p class="sub">Computed and transcribed readings you save will appear here.</p>
    </div>
  {:else}
    <ul class="list">
      {#each entries as e (e.dir)}
        <li>
          <button
            type="button"
            class="row"
            onclick={() => openReading(e.chartPath)}
            disabled={app.busy !== false}
          >
            <span class="who">
              <span class="name">{e.name || "untitled"}</span>
              <span class="apparatus-text sub"
                >{e.born}{e.born && e.place ? " · " : ""}{e.place}</span
              >
            </span>
            <span class="meta apparatus-text">
              {e.excerpts}
              {e.excerpts === 1 ? "passage" : "passages"}{e.modifiedMs
                ? ` · saved ${savedLabel(e.modifiedMs)}`
                : ""}
            </span>
          </button>
          <button
            type="button"
            class="remove"
            title="remove from library"
            aria-label={`remove ${e.name || "this"}'s reading`}
            onclick={(ev) => remove(ev, e)}
          >
            ✕
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="foot">
    <button type="button" class="ghost" onclick={openFile} disabled={app.busy !== false}>
      open a file…
    </button>
    <span class="sep" aria-hidden="true">·</span>
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
  .sep {
    color: var(--ink-3);
    margin: 0 0.6rem;
  }
</style>
