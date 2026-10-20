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
  import { defaults, locales, setModelPath } from "$lib/options.svelte";
  import { SIDEREAL } from "$lib/types";
  import { notify } from "$lib/toasts.svelte";
  import { clearNotes, notes, refreshNotes } from "$lib/notes.svelte";
  import CalcOptions from "./CalcOptions.svelte";
  import MomentField from "./MomentField.svelte";
  import PlacePicker from "./PlacePicker.svelte";
  import type { PlaceDto } from "$lib/types";
  import { onMount } from "svelte";
  import { basename } from "$lib/files";
  import { reason } from "$lib/failure";
  import { ensureReadingsDir } from "$lib/readings";

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
  let zodiac = $state(initial?.zodiac ?? defaults().zodiac);
  /* svelte-ignore state_referenced_locally */
  let ayanamsa = $state(initial?.ayanamsa ?? "");
  let error = $state("");


  // the preferred model / default language prefill untouched fields; the
  // calculator's `initial` values were seeded above, so they win
  onMount(async () => {
    const p = await getPreferences();
    if (!model.trim() && p.default_model) model = p.default_model;
    if (!lang) lang = p.default_locale ?? "en";
    if (!houseSystem) houseSystem = p.default_house_system ?? defaults().houseSystem;
    if (!ayanamsa) ayanamsa = p.default_ayanamsa ?? defaults().ayanamsa;
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
    // In reading order, and all three the same way: the pickers hand back a
    // complete value or an empty string, so "incomplete" is now something this
    // form can see and answer inline rather than send off and be told about.
    if (!date) {
      error = "Enter a birth date.";
      return;
    }
    if (!time) {
      error = "Enter a birth time.";
      return;
    }
    if (!picked) {
      error = "Pick the place from the suggestions.";
      return;
    }
    const place = picked;
    // Building a reading is deciding to keep one, so this is the moment a
    // readings folder is earned. Asked before the build rather than after, so
    // the answer is in place when `build` looks for somewhere to save — and
    // declining is fine: the reading is then held in the session and exported
    // by hand, exactly as it was before this asked at all.
    await ensureReadingsDir();
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
          ayanamsa: zodiac === SIDEREAL ? ayanamsa || null : null,
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
        // The take's notes are in the store, not on the chart — read them
        // back rather than counting a field that no longer exists.
        await refreshNotes();
        const n = notes().length;
        notify(`${n} ${n === 1 ? "note" : "notes"} routed past the verify gate`);
      } else {
        clearNotes();
      }
      setModelPath(model.trim());
    } catch (e) {
      error = reason(e);
    }
  }
</script>

<!-- The sheet's head (Overlay's `title`) names it; this is only the fields.
     In the order a birth certificate gives them, the optional apparatus for a
     recorded session last. -->
<form
  class="form"
  onsubmit={(e) => {
    e.preventDefault();
    compute();
  }}
>
  <label class="fld">
    <span class="lbl">Name</span>
    <input bind:value={name} placeholder="The chart holder's name" />
  </label>
  <div class="row">
    <div class="fld">
      <label class="lbl" for="f-date">Born on</label>
      <MomentField id="f-date" type="date" value={date} width="100%" oncommit={(v) => (date = v)} />
    </div>
    <div class="fld">
      <label class="lbl" for="f-time">At</label>
      <MomentField id="f-time" type="time" value={time} width="100%" oncommit={(v) => (time = v)} />
    </div>
  </div>
  <label class="fld place">
    <span class="lbl">Place of birth</span>
    <PlacePicker bind:value={picked} placeholder="Type a city" />
  </label>
  <div class="row">
    <label class="fld">
      <span class="lbl">Language</span>
      <select bind:value={lang}>
        {#each locales as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
    <CalcOptions bind:houseSystem bind:zodiac bind:ayanamsa />
  </div>

  <h3 class="group-head">Recording <span class="opt">— optional</span></h3>
  <label class="fld">
    <span class="lbl">Transcript or audio</span>
    <span class="with-act">
      <input bind:value={transcript} placeholder=".txt, .jsonl, or a .wav to transcribe" />
      <button type="button" class="btn quiet" onclick={() => pickFile("transcript")}>Browse…</button>
    </span>
  </label>
  <label class="fld">
    <span class="lbl">Whisper model <span class="opt">— needed for audio</span></span>
    <span class="with-act">
      <span class="model-slot">
        <input
          readonly
          value={model ? basename(model) : ""}
          placeholder="No model chosen"
          title={model || undefined}
          onclick={() => pickFile("model")}
          onkeydown={(e) => e.key === "Enter" && pickFile("model")}
        />
        {#if model}
          <button type="button" class="clear" aria-label="clear the model" onclick={() => (model = "")}>
            <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" /></svg>
          </button>
        {/if}
      </span>
      <button type="button" class="btn quiet" onclick={() => pickFile("model")}>Browse…</button>
    </span>
  </label>

  <div class="actions">
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <button type="submit" class="btn primary compute" disabled={isBusy()}>
      {#if phase.kind === "transcribe"}
        Transcribing… {phase.pct}%
      {:else if phase.kind === "compute"}
        Computing the chart…
      {:else}
        Compute the chart
      {/if}
    </button>
    {#if phase.kind === "transcribe"}
      <div class="bar"><div class="fill" style="width: {phase.pct}%"></div></div>
    {/if}
  </div>
</form>

<style>
  .form {
    padding-top: 0.4rem;
  }
  .place :global(.picker) {
    display: block;
  }
  /* the model shows its NAME (the path is a tooltip); the readonly field is
     itself a picker, with a quiet clear mark inside its right end */
  .model-slot {
    position: relative;
    display: flex;
    min-width: 0;
  }
  .model-slot input {
    cursor: pointer;
    padding-right: 2rem;
  }
  .clear {
    position: absolute;
    top: 50%;
    right: 0.4rem;
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
  }
  .clear svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
  }
  .clear:hover {
    color: var(--ink);
    background: var(--surface-hover);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    margin-top: 0.6rem;
  }
  .compute {
    width: 100%;
    height: 2.6rem;
    font-size: 0.92rem;
  }
  .bar {
    height: 3px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--brass);
    transition: width 0.4s var(--ease-out-quint);
  }
  @media (prefers-reduced-motion: reduce) {
    .fill {
      transition: none;
    }
  }
</style>
