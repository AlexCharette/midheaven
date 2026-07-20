<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { build, onTranscribeProgress, searchPlaces } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { PlaceDto } from "$lib/types";
  import { onMount } from "svelte";

  let name = $state("");
  let date = $state("");
  let time = $state("");
  let placeQuery = $state("");
  let picked = $state<PlaceDto | null>(null);
  let suggestions = $state<PlaceDto[]>([]);
  let sel = $state(0);
  let transcript = $state("");
  let model = $state("");
  let error = $state("");

  onMount(() => {
    const unlisten = onTranscribeProgress((pct) => (app.progress = pct));
    return () => {
      unlisten.then((f) => f());
    };
  });

  async function queryPlaces() {
    picked = null;
    const q = placeQuery.trim();
    suggestions = q ? await searchPlaces(q) : [];
    sel = 0;
  }

  function pick(p: PlaceDto) {
    picked = p;
    placeQuery = `${p.label} · ${p.tz}`;
    suggestions = [];
  }

  function onPlaceKey(e: KeyboardEvent) {
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
      suggestions = [];
    }
  }

  async function pickFile(kind: "transcript" | "model") {
    const filters =
      kind === "transcript"
        ? [{ name: "transcript or audio", extensions: ["txt", "jsonl", "wav"] }]
        : [{ name: "ggml model", extensions: ["bin"] }];
    const path = await open({ multiple: false, filters });
    if (typeof path === "string") {
      if (kind === "transcript") transcript = path;
      else model = path;
    }
  }

  async function compute() {
    error = "";
    if (!picked) {
      error = "pick a place from the suggestions";
      return;
    }
    app.building = true;
    app.progress = null;
    try {
      app.chart = await build({
        name,
        date,
        time,
        place_id: picked.id,
        transcript: transcript || null,
        model: model || null,
      });
      app.status = `${app.chart.excerpts.length} passages routed past the verify gate`;
    } catch (e) {
      error = String(e);
    } finally {
      app.building = false;
      app.progress = null;
    }
  }
</script>

<div class="plate">
  <div class="ornament">✶</div>
  <p class="apparatus-text">The Nativity Desk</p>
  <h1>ASTRO</h1>
  <p class="apparatus-text">enter the birth data; the chart computes entirely offline</p>

  <form
    onsubmit={(e) => {
      e.preventDefault();
      compute();
    }}
  >
    <label>
      <span>name</span>
      <input bind:value={name} placeholder="the chart holder's name" />
    </label>
    <label>
      <span>born on</span>
      <input bind:value={date} placeholder="YYYY-MM-DD" />
    </label>
    <label>
      <span>at</span>
      <input bind:value={time} placeholder="HH:MM, local civil time" />
    </label>
    <label class="place">
      <span>in</span>
      <input
        bind:value={placeQuery}
        oninput={queryPlaces}
        onkeydown={onPlaceKey}
        placeholder="type a city — the gazetteer will offer choices"
      />
      {#if suggestions.length > 0}
        <ul class="dropdown">
          {#each suggestions as p, i (p.id)}
            <li>
              <button type="button" class:current={i === sel} onclick={() => pick(p)}>
                <span class="marker">{i === sel ? "☞" : ""}</span>{p.label}
                <span class="apparatus-text">{p.tz}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </label>
    <label>
      <span>transcript</span>
      <input bind:value={transcript} placeholder="path to .txt, .jsonl — or a .wav to transcribe (optional)" />
      <button type="button" class="browse" onclick={() => pickFile("transcript")}>browse…</button>
    </label>
    <label>
      <span>model</span>
      <input bind:value={model} placeholder="ggml whisper model, needed for audio" />
      <button type="button" class="browse" onclick={() => pickFile("model")}>browse…</button>
    </label>

    {#if error}<p class="error">✗ {error}</p>{/if}

    <div class="actions">
      <button type="submit" class="compute" disabled={app.building}>
        {#if app.building && app.progress !== null}
          transcribing… {app.progress}%
        {:else if app.building}
          computing the figure…
        {:else}
          compute the figure
        {/if}
      </button>
    </div>
    {#if app.building && app.progress !== null}
      <div class="bar"><div class="fill" style="width: {app.progress}%"></div></div>
    {/if}
  </form>
</div>

<style>
  .plate {
    max-width: 46rem;
    margin: 0 auto;
    padding: 3.5rem 1.5rem;
    text-align: center;
  }
  .ornament {
    color: var(--ink-2);
    opacity: 0.75;
    font-size: 1.4rem;
  }
  h1 {
    font-weight: 500;
    letter-spacing: 0.34em;
    text-indent: 0.34em;
    font-size: 2.2rem;
    margin: 0.2rem 0 0.4rem;
  }
  form {
    margin-top: 2rem;
    text-align: left;
  }
  label {
    display: grid;
    grid-template-columns: 7.5rem 1fr auto;
    gap: 0 1rem;
    align-items: baseline;
    margin-bottom: 1.1rem;
    position: relative;
  }
  label span:first-child {
    font-style: italic;
    color: var(--ink-3);
    text-align: right;
  }
  .browse {
    font-size: 0.85rem;
    color: var(--ink-3);
    font-style: italic;
  }
  .browse:hover {
    color: var(--ink);
  }
  .dropdown {
    position: absolute;
    top: 100%;
    left: 8.5rem;
    right: 0;
    z-index: 10;
    margin: 0.2rem 0 0;
    padding: 0.3rem 0;
    list-style: none;
    background: var(--bg-deep);
    border: 1px solid var(--hairline);
  }
  .dropdown button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 0.15rem 0.7rem;
    color: var(--ink-2);
  }
  .dropdown button .marker {
    display: inline-block;
    width: 1.2em;
    color: var(--ink-2);
  }
  .dropdown button.current,
  .dropdown button:hover {
    color: var(--ink);
    text-decoration: underline;
  }
  .dropdown .apparatus-text {
    margin-left: 0.6em;
    font-size: 0.85em;
  }
  .error {
    color: var(--oxblood);
    font-style: italic;
    text-align: center;
  }
  .actions {
    text-align: center;
    margin-top: 1.6rem;
  }
  .compute {
    border: 1px solid var(--hairline);
    padding: 0.4rem 1.6rem;
    font-variant: small-caps;
    letter-spacing: 0.18em;
    color: var(--ink);
  }
  .compute:hover:not(:disabled) {
    background: rgba(233, 228, 211, 0.08);
  }
  .compute:disabled {
    color: var(--ink-3);
    cursor: wait;
  }
  .bar {
    margin: 1rem auto 0;
    max-width: 20rem;
    height: 2px;
    background: var(--line);
  }
  .fill {
    height: 100%;
    background: var(--brass);
    transition: width 0.4s ease-out;
  }
</style>
