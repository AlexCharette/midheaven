<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import {
    artifactFilename,
    getPreferences,
    onBuildWarnings,
    onTranscribeProgress,
    saveArtifact,
    savePdf,
    startRecording,
    stopRecording,
  } from "$lib/api";
  import {
    chart as openChart,
    isRecording,
    leaveReading,
    takeBegan,
    takeEnded,
    updateChart,
    whyCannotLeave,
    whyCannotRecord,
  } from "$lib/session.svelte";
  import { busy, during, isBusy, setProgress } from "$lib/busy.svelte";
  import { clearNotes, notes, setNotes } from "$lib/notes.svelte";
  import { clearPins, mode, resetFocus, setMode, visibleNotes } from "$lib/focus.svelte";
  import { canRecord, loadCalcOptions, loadLocales, modelPath, setModelPath } from "$lib/options.svelte";
  import { notify } from "$lib/toasts.svelte";
  import { TROPICAL } from "$lib/types";
  import { fmt, loadChrome, t } from "$lib/chrome.svelte";
  import Calculator from "$lib/components/Calculator.svelte";
  import ChartCore from "$lib/components/ChartCore.svelte";
  import Beside from "$lib/components/Beside.svelte";
  import Commentary from "$lib/components/Commentary.svelte";
  import IndexOfElements from "$lib/components/IndexOfElements.svelte";
  import Wheel from "$lib/components/Wheel.svelte";
  import { onMount } from "svelte";
  import { reason } from "$lib/failure";

  const reading = $derived(openChart());
  // Which notes the hover and the pins add up to is the focus module's rule,
  // not this view's — the hover-preview half of it used to live here, apart from
  // the pin rule it completes.
  const visible = $derived(reading ? visibleNotes(notes()) : []);
  // A local binding so the template can narrow the discriminated phase.
  const phase = $derived(busy());

  // transcription progress can arrive during a form build or a live take
  onMount(() => {
    // The reading-language list (endonyms + house suffixes) comes from the
    // backend once; the form and preferences selectors read it from state.
    loadLocales();
    loadCalcOptions();
    loadChrome();
    // A configured default model enables live transcription on ANY open chart,
    // not only ones just built through the form (which sets the path itself) —
    // so a reading opened from the library can still be transcribed onto.
    if (!canRecord()) {
      getPreferences()
        .then((p) => {
          if (!canRecord() && p.default_model) setModelPath(p.default_model);
        })
        .catch(() => {});
    }
    const unlisten = onTranscribeProgress(setProgress);
    // Warnings the pipeline used to write to stderr now surface as toasts.
    const unlistenWarn = onBuildWarnings((ws) => ws.forEach((w) => notify(w)));
    return () => {
      unlisten.then((f) => f());
      unlistenWarn.then((f) => f());
    };
  });

  // ---- live session recording ----
  // Whether a take is running lives in the session, alongside the reading it is
  // being spoken over — the two used to be independent, which is how leaving a
  // reading could strand a recorder. `recSecs` stays here because it is only a
  // display counter, and no path can change the phase underneath it while
  // recording (leaving, opening another reading and recalculating are refused).
  let recSecs = $state(0);
  let recTimer: ReturnType<typeof setInterval> | undefined;
  const mmss = (s: number) =>
    `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;

  async function toggleRecording() {
    if (!isRecording()) {
      try {
        // The phase turns only after the backend confirms a recorder is up.
        await startRecording(modelPath());
        if (!takeBegan()) return;
        recSecs = 0;
        recTimer = setInterval(() => recSecs++, 1000);
        notify("listening — speak the reading; stop to route it");
      } catch (e) {
        notify(reason(e), "error");
      }
      return;
    }
    clearInterval(recTimer);
    // The recorder is gone the moment `stop_recording` is entered, so the phase
    // leaves `recording` before the transcription await rather than after: the
    // footer should show progress, not a stop button for a recorder that has
    // already ended. A failed transcription therefore keeps the chart it had.
    takeEnded(null);
    notify("routing the recording…");
    try {
      // `transcribe` rather than `compute`: the backend reports whole-percent
      // progress through `setProgress`, which only lands in that phase.
      // A landed take hands back the reading's notes, not a chart: the sky
      // did not change, only what was said about it.
      const routed = await during("transcribe", stopRecording);
      setNotes(routed);
      // English: this whole sentence has no Russian yet, like the forms'.
      notify(`${routed.length} notes on the chart`);
    } catch (e) {
      notify(reason(e), "error");
    }
  }

  async function engrave() {
    const path = await save({
      // generated `{name}_{date}.html`, matching the library folder
      defaultPath: await artifactFilename().catch(() => "reading.html"),
      filters: [{ name: "HTML artifact", extensions: ["html"] }],
    });
    if (!path) return;
    try {
      const written = await saveArtifact(path);
      notify(`wrote ${written} ☞ open it in a browser`);
    } catch (e) {
      notify(reason(e), "error");
    }
  }

  async function engravePdf() {
    const suggested = await artifactFilename().catch(() => "reading.html");
    const path = await save({
      defaultPath: suggested.replace(/\.html$/, ".pdf"),
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!path) return;
    try {
      const written = await savePdf(path);
      notify(`wrote ${written}`);
    } catch (e) {
      notify(reason(e), "error");
    }
  }

  // Refused mid-take — the stop control lives in this view, so leaving would
  // strand a recorder the user can no longer reach and its take would land on
  // whatever reading is opened next. The button is disabled for the same reason;
  // this is the backstop, and it surfaces the refusal rather than failing quietly.
  function back() {
    if (!leaveReading()) return;
    resetFocus();
    clearNotes();
  }
</script>

{#if reading}
  <div class="reading" data-posture={isRecording() ? "recording" : "curating"}>
    <figure class="plate">
      <div class="plate-frame">
        <!-- keyed on the calculation so a live reproject remounts the wheel and
             replays its ring-draw/rise-in entrance as the transition; ChartCore
             stays mounted so the caption control keeps focus across the swap -->
        {#key reading.meta.house_system + "|" + (reading.meta.ayanamsa ?? TROPICAL)}
          <Wheel chart={reading} notes={notes()} />
        {/key}
        <ChartCore chart={reading} notes={notes()} />
      </div>
      <!-- the index is the plate's legend, not a region competing with the
           commentary: it belongs to the figure it annotates, the way a key
           sits under an engraved plate rather than beside it -->
      <figcaption class="legend">
        <IndexOfElements chart={reading} />
      </figcaption>
    </figure>

    <!-- one continuous ruled stream: filter state, commentary and prior
         notes separated by rule weight and type, never by enclosure -->
    <section class="stream">
      <p class="station">
        <span class="apparatus-text">{t().notesTouching}</span>
        <span class="detents">
          <button
            class="detent"
            aria-pressed={mode() === "any"}
            onclick={() => setMode("any")}
            title={t().anyTitle}>{t().any}</button
          ><span class="detent-sep" aria-hidden="true">/</span><button
            class="detent"
            aria-pressed={mode() === "all"}
            onclick={() => setMode("all")}
            title={t().allTitle}>{t().all}</button
          >
        </span>
        <span class="apparatus-text">{t().ofSelection}</span>
        <button class="ghost" onclick={clearPins}>{t().clear}</button>
        <span class="count apparatus-text">{fmt(t().count, { shown: visible.length, total: notes().length })}</span>
      </p>

      <Commentary chart={reading} {visible} />

      <Beside chart={reading} />
    </section>
  </div>
  <footer>
    <button
      class="ghost"
      onclick={back}
      disabled={whyCannotLeave() !== null}
      title={whyCannotLeave() ?? undefined}>← new reading</button
    >
    <span class="foot-actions">
      {#if canRecord()}
        <button
          class="mark-btn rec"
          class:on={isRecording()}
          onclick={toggleRecording}
          disabled={!isRecording() && (isBusy() || whyCannotRecord() !== null)}
        >
          {#if isRecording()}
            <span class="dot" aria-hidden="true"></span> stop transcribing · {mmss(recSecs)}
          {:else if phase.kind === "transcribe"}
            transcribing… {phase.pct}%
          {:else}
            ◉ begin transcribing
          {/if}
        </button>
      {/if}
      <span class="export-group">
        <span class="apparatus-text export-lbl">export</span>
        <button
          class="frame-btn primary"
          onclick={engrave}
          disabled={isRecording()}
          title="the self-contained HTML reading — opens in any browser"
        >HTML</button>
        <button
          class="mark-btn"
          onclick={engravePdf}
          disabled={isRecording()}
          title="a printer-friendly PDF"
        >PDF</button>
      </span>
    </span>
  </footer>
{:else}
  <!-- the landing view: the live chart calculator; the birth form now lives
       in its fly-out -->
  <Calculator />
{/if}

<style>
  /* Two postures, one page.
     The astrologer works here in two scenes that want opposite things: a live
     call, where the chart is what they are looking at while they talk, and the
     curation afterwards, where the words are. Rather than compromise into a
     layout that serves neither, the composition shifts: recording widens the
     plate and lets the apparatus recede; stopping gives the stream its room
     back. It is the page's one authored moment of motion. */
  .reading {
    display: grid;
    grid-template-columns: minmax(460px, 48fr) minmax(0, 52fr);
    gap: clamp(1.6rem, 3vw, 3.2rem);
    padding: 1.4rem clamp(1.2rem, 3vw, 2.6rem) 4.8rem;
    max-width: 1580px;
    margin: 0 auto;
  }
  /* While a take runs there is nothing new to read — the words arrive in a
     batch when it stops — so the page stops pretending otherwise and gives
     the room to the thing actually in use. */
  .reading[data-posture="recording"] {
    grid-template-columns: minmax(460px, 64fr) minmax(0, 36fr);
  }
  .reading[data-posture="recording"] .stream {
    opacity: 0.58;
  }
  .reading[data-posture="recording"] .legend {
    opacity: 0;
    pointer-events: none;
  }
  @media (prefers-reduced-motion: no-preference) {
    .reading {
      transition: grid-template-columns var(--dur-slow) var(--ease-out-expo);
    }
    .stream,
    .legend {
      transition:
        opacity var(--dur-slow) var(--ease-out-expo),
        filter var(--dur-slow) var(--ease-out-expo);
    }
  }
  .plate {
    margin: 0;
    position: sticky;
    top: 0.9rem;
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
  }
  /* The legend hangs off the plate, inset from its frame the way a key is
     inset from the engraving it explains. */
  .legend {
    padding: 0 0.6rem;
  }
  /* the wheel's plate uses a tighter padding than the shared primitive; it also
     anchors the hub read-out core, which is absolutely centred over the wheel */
  .plate-frame {
    position: relative;
    border: 1px solid var(--hairline);
    outline: 1px solid var(--line);
    outline-offset: 5px;
    padding: 0.8rem;
    margin: 6px;
    background: radial-gradient(ellipse at 50% 42%, var(--plate-glow) 0%, transparent 70%);
  }
  /* below the split point the plate leads, stacked above the reading column */
  @media (max-width: 900px) {
    .reading,
    .reading[data-posture="recording"] {
      grid-template-columns: 1fr;
      gap: 1.6rem;
    }
    .plate {
      position: static;
      max-width: 560px;
      width: 100%;
      margin: 0 auto;
    }
    /* Stacked, the plate already has the viewport to itself while the take
       runs, so dimming what is below it would only look broken. */
    .reading[data-posture="recording"] .stream,
    .reading[data-posture="recording"] .legend {
      opacity: 1;
      pointer-events: auto;
    }
  }
  /* The stream's first line, not a toolbar. It reads as a sentence —
     "notes touching any / all of the selection · 3 of 12" — and is held to
     the column by a single rule beneath it rather than boxed between two. */
  .station {
    display: flex;
    align-items: baseline;
    gap: 0.55rem;
    flex-wrap: wrap;
    margin: 0 0 1.5rem;
    padding: 0 0 0.6rem;
    border-bottom: 1px solid var(--line);
    font-size: 0.9rem;
  }
  /* A detent is a printed mark, not a button in a box: the chosen reading is
     underlined in brass, the other is quiet. */
  .detents {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3rem;
  }
  .detent {
    color: var(--ink-3);
    font-variant: small-caps;
    letter-spacing: 0.1em;
    padding: 0 0.1em 0.12em;
    border-bottom: 1px solid transparent;
  }
  .detent:hover {
    color: var(--ink-2);
  }
  .detent[aria-pressed="true"] {
    color: var(--ink);
    border-bottom-color: var(--brass);
  }
  .detent-sep {
    color: var(--ink-3);
    opacity: 0.6;
  }
  @media (prefers-reduced-motion: no-preference) {
    .detent {
      transition:
        color var(--dur-fast) var(--ease-out-quint),
        border-bottom-color var(--dur-fast) var(--ease-out-quint);
    }
  }
  /* the count is the only thing that reaches this column's right edge, and at
     wide widths that edge is where the layout's fixed preferences gear sits —
     so the toolbar keeps a corner's worth of room clear */
  .count {
    margin-left: auto;
    margin-right: 2.4rem;
    font-variant-numeric: tabular-nums;
  }
  /* The console rail: one hairline, the page's width, and the actions set on
     it as marks rather than a row of boxes. */
  footer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: var(--z-footer);
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem clamp(1.2rem, 3vw, 2.6rem);
    background: var(--bg-deep);
    border-top: 1px solid var(--line);
    font-size: 0.88rem;
  }
  .foot-actions {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 1.2rem;
  }
  /* the export pair reads as one grouped act, set off from the recessive
     "new reading" / record buttons by a hairline; HTML is the primary. */
  .export-group {
    display: inline-flex;
    align-items: baseline;
    gap: 0.75rem;
    padding-left: 1.3rem;
    border-left: 1px solid var(--line);
  }
  .export-lbl {
    font-variant: small-caps;
    letter-spacing: 0.12em;
  }
  .frame-btn.primary {
    border-color: var(--brass);
    transition: background var(--dur-base) var(--ease-out-quint);
  }
  .frame-btn.primary:hover:not(:disabled) {
    background: var(--brass-wash);
  }
  .rec.on {
    color: var(--ink);
    border-bottom-color: var(--brass);
  }
  .dot {
    display: inline-block;
    width: 0.55em;
    height: 0.55em;
    border-radius: 50%;
    background: var(--brass);
    animation: pulse 1.6s ease-out infinite;
  }
  @media (prefers-reduced-motion: reduce) {
    .dot {
      animation: none;
    }
  }
  @keyframes pulse {
    0% {
      opacity: 1;
    }
    50% {
      opacity: 0.25;
    }
    100% {
      opacity: 1;
    }
  }
</style>
