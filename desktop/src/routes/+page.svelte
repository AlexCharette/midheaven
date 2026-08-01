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
  import { clearPins, mode, resetFocus, setMode, visibleExcerpts } from "$lib/focus.svelte";
  import { canRecord, loadCalcOptions, loadLocales, modelPath, setModelPath } from "$lib/options.svelte";
  import { notify } from "$lib/toasts.svelte";
  import { TROPICAL } from "$lib/types";
  import { fmt, loadChrome, t } from "$lib/chrome.svelte";
  import Calculator from "$lib/components/Calculator.svelte";
  import ChartCore from "$lib/components/ChartCore.svelte";
  import Commentary from "$lib/components/Commentary.svelte";
  import IndexOfElements from "$lib/components/IndexOfElements.svelte";
  import Wheel from "$lib/components/Wheel.svelte";
  import { onMount } from "svelte";
  import { reason } from "$lib/failure";

  const reading = $derived(openChart());
  // Which passages the hover and the pins add up to is the focus module's rule,
  // not this view's — the hover-preview half of it used to live here, apart from
  // the pin rule it completes.
  const visible = $derived(reading ? visibleExcerpts(reading) : []);
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
      const routed = await during("transcribe", stopRecording);
      updateChart(routed);
      // English: this whole sentence has no Russian yet, like the forms'.
      notify(`${routed.excerpts.length} passages on the chart`);
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
  }
</script>

{#if reading}
  <div class="reading">
    <figure class="plate">
      <div class="plate-frame">
        <!-- keyed on the calculation so a live reproject remounts the wheel and
             replays its ring-draw/rise-in entrance as the transition; ChartCore
             stays mounted so the caption control keeps focus across the swap -->
        {#key reading.meta.house_system + "|" + (reading.meta.ayanamsa ?? TROPICAL)}
          <Wheel chart={reading} />
        {/key}
        <ChartCore chart={reading} />
      </div>
    </figure>

    <section>
      <div class="toolbar">
        <span class="apparatus-text">{t().passagesTouching}</span>
        <span class="segmented">
          <button aria-pressed={mode() === "any"} onclick={() => setMode("any")} title={t().anyTitle}>{t().any}</button>
          <button aria-pressed={mode() === "all"} onclick={() => setMode("all")} title={t().allTitle}>{t().all}</button>
        </span>
        <span class="apparatus-text">{t().ofSelection}</span>
        <button class="ghost" onclick={clearPins}>{t().clear}</button>
        <span class="count apparatus-text">{fmt(t().count, { shown: visible.length, total: reading.excerpts.length })}</span>
      </div>

      <IndexOfElements chart={reading} />

      <Commentary chart={reading} {visible} />
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
          class="frame-btn rec"
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
          class="frame-btn"
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
  /* the wheel is the hero plate; the apparatus and commentary are its caption */
  .reading {
    display: grid;
    grid-template-columns: minmax(520px, 60%) minmax(0, 1fr);
    gap: 2rem;
    padding: 1rem 1.6rem 4.4rem 1.1rem;
    max-width: 1580px;
    margin: 0 auto;
  }
  .plate {
    margin: 0;
    position: sticky;
    top: 0.9rem;
    align-self: start;
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
    .reading {
      grid-template-columns: 1fr;
      gap: 1.6rem;
    }
    .plate {
      position: static;
      max-width: 560px;
      width: 100%;
      margin: 0 auto;
    }
  }
  .toolbar {
    display: flex;
    align-items: baseline;
    gap: 0.9rem;
    flex-wrap: wrap;
    padding: 0.7rem 0;
    margin-bottom: 1.4rem;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    font-size: 0.9rem;
  }
  .segmented {
    display: inline-flex;
    border: 1px solid var(--hairline);
  }
  .segmented button {
    padding: 0.12rem 0.8rem;
    color: var(--ink-2);
    font-variant: small-caps;
    letter-spacing: 0.1em;
  }
  .segmented button + button {
    border-left: 1px solid var(--hairline);
  }
  .segmented button[aria-pressed="true"] {
    background: var(--ink);
    color: var(--bg-deep);
  }
  .count {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
  footer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: var(--z-footer);
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.45rem 1.4rem;
    background: var(--bg-deep);
    border-top: 1px solid var(--hairline);
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
    align-items: center;
    gap: 0.6rem;
    padding-left: 1.2rem;
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
    border-color: var(--brass);
    color: var(--ink);
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
