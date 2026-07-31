<script lang="ts">
  // The birth-chart form — lives in the calculator's fly-out. `initial` seeds
  // it with the moment being explored (the calculator's date/time/place and
  // calculation choices); submitting builds a full reading and flips the app
  // into the reading view.
  import { open } from "@tauri-apps/plugin-dialog";
  import { build, getPreferences, setLastPlace } from "$lib/api";
  import { openReading } from "$lib/session.svelte";
  import { busy, during, isBusy } from "$lib/busy.svelte";
  import { resetFocus } from "$lib/focus.svelte";
  import { locales, setModelPath } from "$lib/options.svelte";
  import { notify } from "$lib/toasts.svelte";
  import CalcOptions from "./CalcOptions.svelte";
  import PlacePicker from "./PlacePicker.svelte";
  import type { PlaceDto } from "$lib/types";
  import { onMount } from "svelte";
  import { reason } from "$lib/failure";

  type Initial = Partial<{
    date: string;
    time: string;
    place: PlaceDto | null;
    houseSystem: string;
    zodiac: string;
    ayanamsa: string;
  }>;

  let {
    initial,
    onclose,
  }: {
    initial?: Initial;
    onclose?: () => void;
  } = $props();

  // `initial` is a seed by design — the form captures the calculator's moment
  // at open and then owns its own edits, so the captures below are one-shot.
  /* svelte-ignore state_referenced_locally */
  let name = $state("");
  /* svelte-ignore state_referenced_locally */
  let date = $state(initial?.date ?? "");
  /* svelte-ignore state_referenced_locally */
  let time = $state(initial?.time ?? "");
  /* svelte-ignore state_referenced_locally */
  let picked = $state<PlaceDto | null>(initial?.place ?? null);
  let transcript = $state("");
  let model = $state("");
  let lang = $state("");
  /* svelte-ignore state_referenced_locally */
  let houseSystem = $state(initial?.houseSystem ?? "");
  /* svelte-ignore state_referenced_locally */
  let zodiac = $state(initial?.zodiac ?? "tropical");
  /* svelte-ignore state_referenced_locally */
  let ayanamsa = $state(initial?.ayanamsa ?? "");
  let error = $state("");

  // the model is picked as a file but read as a name — the path is backend
  // detail, the basename is what a person recognizes
  const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;

  // the preferred model / default language prefill untouched fields; the
  // calculator's `initial` values were seeded above, so they win
  onMount(async () => {
    const p = await getPreferences();
    if (!model.trim() && p.default_model) model = p.default_model;
    if (!lang) lang = p.default_locale ?? "en";
    if (!houseSystem) houseSystem = p.default_house_system ?? "whole-sign";
    if (!ayanamsa) ayanamsa = p.default_ayanamsa ?? "lahiri";
    // Zodiac is a real toggle (default tropical); a set preference moves it
    // only when the calculator didn't hand one over.
    if (initial?.zodiac === undefined && p.default_zodiac) zodiac = p.default_zodiac;
  });

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

  // A local binding so the template can narrow the discriminated phase; a
  // bare `busy()` call cannot be narrowed across two reads.
  const phase = $derived(busy());

  async function compute() {
    error = "";
    if (!picked) {
      error = "pick a place from the suggestions";
      return;
    }
    const place = picked;
    try {
      const chart = await during("compute", () =>
        build({
          name,
          date,
          time,
          place_id: place.id,
          transcript: transcript || null,
          model: model || null,
          lang: lang || null,
          house_system: houseSystem || null,
          zodiac: zodiac || null,
          ayanamsa: zodiac === "sidereal" ? ayanamsa || null : null,
        }),
      );
      // a built reading is the strongest "last used place" signal
      setLastPlace(place.id).catch(() => {});
      // the calculator's plate shares the focus — a fresh reading must not
      // inherit stale pins or a stale hover
      resetFocus();
      openReading(chart);
      // Only worth announcing the routing when a transcript was actually
      // supplied; a bare chart with no transcript routes nothing.
      if (transcript.trim()) {
        const n = chart.excerpts.length;
        notify(`${n} ${n === 1 ? "passage" : "passages"} routed past the verify gate`);
      }
      setModelPath(model.trim());
    } catch (e) {
      error = reason(e);
    }
  }
</script>

<p class="rubric">cast a natal chart</p>
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
  <div class="duo">
    <label class="lbl" for="f-date">born on</label>
    <input id="f-date" bind:value={date} placeholder="YYYY-MM-DD" />
    <label class="lbl" for="f-time">at</label>
    <input id="f-time" bind:value={time} placeholder="HH:MM" title="24-hour clock" />
  </div>
  <label class="place">
    <span>in</span>
    <PlacePicker bind:value={picked} />
  </label>
  <label>
    <span>language</span>
    <select class="lang" bind:value={lang}>
      {#each locales as l (l.code)}
        <option value={l.code}>{l.label}</option>
      {/each}
    </select>
  </label>
  <CalcOptions bind:houseSystem bind:zodiac bind:ayanamsa />
  <label>
    <span>transcript</span>
    <input bind:value={transcript} placeholder=".txt / .jsonl — or a .wav to transcribe (optional)" />
    <button type="button" class="browse" onclick={() => pickFile("transcript")}>browse…</button>
  </label>
  <label>
    <span>model</span>
    <span class="model-slot">
      <input
        readonly
        value={model ? basename(model) : ""}
        placeholder="ggml whisper model — for audio (optional)"
        title={model || undefined}
        onclick={() => pickFile("model")}
        onkeydown={(e) => e.key === "Enter" && pickFile("model")}
      />
      {#if model}
        <button type="button" class="clear" aria-label="clear the model" onclick={() => (model = "")}>×</button>
      {/if}
    </span>
    <button type="button" class="browse" onclick={() => pickFile("model")}>browse…</button>
  </label>

  {#if error}<p class="error">✗ {error}</p>{/if}

  <div class="actions">
    <button type="submit" class="frame-btn compute" disabled={isBusy()}>
      {#if phase.kind === "transcribe"}
        transcribing… {phase.pct}%
      {:else if phase.kind === "compute"}
        computing the chart…
      {:else}
        compute the chart
      {/if}
    </button>
  </div>
  {#if phase.kind === "transcribe"}
    <div class="bar"><div class="fill" style="width: {phase.pct}%"></div></div>
  {/if}

  {#if onclose}
    <p class="close-line">
      <button type="button" class="ghost" onclick={onclose}>← back to the calculator</button>
    </p>
  {/if}
</form>

<style>
  form {
    margin-top: 1.1rem;
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
    margin-left: 1rem;
  }
  .duo label[for="f-time"] {
    margin-left: 0.6rem; /* pressed up against the date */
  }
  /* the time needs only five characters; sized to them and kept snug against
     the date so the pair reads as one clause and never clips in the fly-out */
  .duo input#f-time {
    width: 5.5rem;
    margin-left: 0.6rem;
  }
  .browse {
    font-size: 0.85rem;
    color: var(--ink-3);
    font-style: italic;
  }
  .browse:hover {
    color: var(--ink);
  }
  /* the model shows its NAME (the path is a tooltip); the readonly input is
     itself a picker, with a quiet × to unset it */
  .model-slot {
    position: relative;
    display: flex;
    align-items: baseline;
    min-width: 0;
  }
  .model-slot input {
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }
  .model-slot .clear {
    position: absolute;
    right: 0.15rem;
    color: var(--ink-3);
    font-size: 0.9rem;
    padding: 0 0.2rem;
  }
  .model-slot .clear:hover {
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
  .error {
    color: var(--oxblood);
    font-style: italic;
    text-align: center;
  }
  .actions {
    text-align: center;
    margin-top: 1rem;
  }
  /* the panel's primary act: a brass-framed plate that fills on hover */
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
  .close-line {
    text-align: center;
    margin-top: 1.2rem;
    font-size: 0.85rem;
  }
</style>
