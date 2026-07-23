<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { build, getPreferences, searchPlaces } from "$lib/api";
  import { app, notify } from "$lib/state.svelte";
  import Library from "./Library.svelte";
  import Preferences from "./Preferences.svelte";
  import WheelMark from "./WheelMark.svelte";
  import { LOCALES, type PlaceDto } from "$lib/types";
  import { fade } from "svelte/transition";

  const reduceMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const swap = { duration: reduceMotion ? 0 : 200 };

  let name = $state("");
  let date = $state("");
  let time = $state("");
  let placeQuery = $state("");
  let picked = $state<PlaceDto | null>(null);
  let suggestions = $state<PlaceDto[]>([]);
  let sel = $state(0);
  let transcript = $state("");
  let model = $state("");
  let lang = $state("");
  let error = $state("");
  let prefsOpen = $state(false);
  let libraryOpen = $state(false);

  // the preferred model / default language prefill untouched fields (again on
  // pane close, so a freshly chosen default lands without retyping)
  async function prefillModel() {
    const p = await getPreferences();
    if (!model.trim() && p.default_model) model = p.default_model;
    if (!lang) lang = p.default_locale ?? "en";
  }
  $effect(() => {
    if (!prefsOpen) prefillModel();
  });

  // monotonic counter: a slow stale response must not overwrite a newer one
  // or re-open a dropdown the user already resolved
  let latest = 0;
  async function queryPlaces() {
    picked = null;
    const seq = ++latest;
    const q = placeQuery.trim();
    const result = q ? await searchPlaces(q) : [];
    if (seq === latest) {
      suggestions = result;
      sel = 0;
    }
  }

  function pick(p: PlaceDto) {
    latest++;
    picked = p;
    placeQuery = p.label;
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
    app.busy = "compute";
    try {
      app.chart = await build({
        name,
        date,
        time,
        place_id: picked.id,
        transcript: transcript || null,
        model: model || null,
        lang: lang || null,
      });
      // Only worth announcing the routing when a transcript was actually
      // supplied; a bare chart with no transcript routes nothing.
      if (transcript.trim()) {
        const n = app.chart.excerpts.length;
        notify(`${n} ${n === 1 ? "passage" : "passages"} routed past the verify gate`);
      }
      app.model = model.trim();
    } catch (e) {
      error = String(e);
    } finally {
      app.busy = false;
    }
  }
</script>

<div class="entry">
  <div class="plate-frame entry-plate">
  <header class="masthead">
    <div class="mark"><WheelMark size={92} /></div>
    <h1>MIDHEAVEN</h1>
    <div class="double-rule"></div>
    <p class="apparatus-text tagline">your offline astrology workbench</p>
  </header>

  {#if prefsOpen}
    <div class="swap" in:fade={swap}>
      <Preferences onclose={() => (prefsOpen = false)} />
    </div>
  {:else if libraryOpen}
    <div class="swap" in:fade={swap}>
      <Library onclose={() => (libraryOpen = false)} />
    </div>
  {:else}
  <form
    in:fade={swap}
    onsubmit={(e) => {
      e.preventDefault();
      compute();
    }}
  >
    <label>
      <span>name</span>
      <input bind:value={name} placeholder="the chart holder's name" />
    </label>
    <div class="duo">
      <label class="lbl" for="f-date">born on</label>
      <input id="f-date" bind:value={date} placeholder="YYYY-MM-DD" />
      <label class="lbl" for="f-time">at</label>
      <input id="f-time" bind:value={time} placeholder="HH:MM · 24h" />
    </div>
    <label class="place">
      <span>in</span>
      <input
        bind:value={placeQuery}
        oninput={queryPlaces}
        onkeydown={onPlaceKey}
        placeholder="type a city"
      />
      {#if suggestions.length > 0}
        <ul class="dropdown">
          {#each suggestions as p, i (p.id)}
            <li>
              <button type="button" class:current={i === sel} onclick={() => pick(p)}>
                <span class="marker">{i === sel ? "☞" : ""}</span>{p.label}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </label>
    <label>
      <span>language</span>
      <select class="lang" bind:value={lang}>
        {#each LOCALES as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>transcript</span>
      <input bind:value={transcript} placeholder=".txt / .jsonl — or a .wav to transcribe (optional)" />
      <button type="button" class="browse" onclick={() => pickFile("transcript")}>browse…</button>
    </label>
    <label>
      <span>model</span>
      <input bind:value={model} placeholder="ggml whisper model — for audio (optional)" />
      <button type="button" class="browse" onclick={() => pickFile("model")}>browse…</button>
    </label>

    {#if error}<p class="error">✗ {error}</p>{/if}

    <div class="actions">
      <button type="submit" class="frame-btn compute" disabled={app.busy !== false}>
        {#if typeof app.busy === "number"}
          transcribing… {app.busy}%
        {:else if app.busy === "compute"}
          computing the chart…
        {:else}
          compute the chart
        {/if}
      </button>
    </div>
    {#if typeof app.busy === "number"}
      <div class="bar"><div class="fill" style="width: {app.busy}%"></div></div>
    {/if}

    <p class="prefs-line">
      <button type="button" class="ghost" onclick={() => (libraryOpen = true)}>
        open a saved reading
      </button>
      <span class="sep" aria-hidden="true">·</span>
      <button type="button" class="ghost" onclick={() => (prefsOpen = true)}>preferences</button>
    </p>
  </form>
  {/if}
  </div>
</div>

<style>
  /* fill the viewport and centre the plate, so the form never scrolls when it
     fits (and still grows past 100vh, staying reachable, on a short window) */
  .entry {
    max-width: 43rem;
    min-height: 100vh;
    margin: 0 auto;
    padding: clamp(0.75rem, 2vh, 1.5rem) 1rem;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  /* the entry plate compresses its block padding so the whole form clears the
     viewport without a scroll */
  .entry-plate {
    text-align: center;
    padding-block: clamp(0.9rem, 2vh, 1.5rem);
  }
  .masthead {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .mark {
    margin-bottom: 0.2rem;
  }
  h1 {
    font-weight: 400;
    letter-spacing: 0.34em;
    text-indent: 0.34em;
    font-size: clamp(1.8rem, 4.5vw, 2.2rem);
    margin: 0.2rem 0 0;
  }
  .tagline {
    margin: 0.45rem 0 0;
  }
  form {
    margin-top: 1.3rem;
    text-align: left;
  }
  label {
    display: grid;
    grid-template-columns: 7.5rem 1fr auto;
    gap: 0 1rem;
    align-items: baseline;
    margin-bottom: 0.7rem;
    position: relative;
  }
  label span:first-child,
  .duo .lbl {
    font-style: italic;
    color: var(--ink-3);
    text-align: right;
  }
  .duo {
    display: grid;
    /* content-width fields packed to the left: date and time both size to
       their own inputs so the time field never stretches to the plate edge,
       and "at" sits snug between them. The trailing space stays empty. */
    grid-template-columns: 7.5rem auto auto auto;
    justify-content: start;
    gap: 0;
    align-items: baseline;
    margin-bottom: 0.7rem;
  }
  .duo input {
    width: 8.5rem;
    margin-left: 1rem; /* same gap after "born on" and after "at" */
  }
  .duo label[for="f-time"] {
    margin-left: 0.4rem; /* pressed up against the date */
  }
  .browse {
    font-size: 0.85rem;
    color: var(--ink-3);
    font-style: italic;
  }
  .browse:hover {
    color: var(--ink);
  }
  .lang {
    justify-self: start;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--line);
    padding: 0.1rem 0.2rem;
    font: inherit;
  }
  .lang option {
    background: var(--bg-deep);
    color: var(--ink);
  }
  .dropdown {
    position: absolute;
    top: 100%;
    left: 8.5rem;
    right: 0;
    z-index: var(--z-dropdown);
    margin: 0.3rem 0 0;
    padding: 0.3rem 0;
    list-style: none;
    background: var(--bg-deep);
    border: 1px solid var(--hairline);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
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
  .error {
    color: var(--oxblood);
    font-style: italic;
    text-align: center;
  }
  .actions {
    text-align: center;
    margin-top: 1rem;
  }
  /* the primary act on the page: a brass-framed plate that fills on hover,
     clearly ahead of the quiet library/preferences links below it. */
  .compute {
    padding: 0.5rem 2rem;
    letter-spacing: 0.18em;
    border-color: var(--brass);
    color: var(--ink);
    transition:
      background var(--dur-base) var(--ease-out-quint),
      box-shadow var(--dur-base) var(--ease-out-quint);
  }
  .compute:hover:not(:disabled) {
    background: var(--brass-wash);
    box-shadow: 0 0 0 1px var(--brass-halo);
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
  .prefs-line {
    text-align: center;
    margin-top: 1.2rem;
    font-size: 0.85rem;
  }
  .prefs-line .sep {
    color: var(--ink-3);
    margin: 0 0.6rem;
  }

  /* The title page settles into place: wordmark, rule, tagline, then the
     form — a gentle upward arrival over the wheel mark's self-draw. */
  @media (prefers-reduced-motion: no-preference) {
    h1,
    .double-rule,
    .tagline {
      opacity: 0;
      animation: settle 0.7s var(--ease-out-quint) forwards;
    }
    h1 {
      animation-delay: 0.18s;
    }
    .double-rule {
      animation-delay: 0.3s;
    }
    .tagline {
      animation-delay: 0.42s;
    }
  }
  @keyframes settle {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
