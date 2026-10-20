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
    writeNote,
  } from "$lib/api";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isPresenting, setPresenting } from "$lib/present.svelte";
  import { elementsOf } from "$lib/types";
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
  import { clearPins, mode, pinCount, pinnedTags, resetFocus, setMode, visibleNotes } from "$lib/focus.svelte";
  import { canRecord, loadCalcOptions, loadLocales, modelPath, setModelPath } from "$lib/options.svelte";
  import { notify } from "$lib/toasts.svelte";
  import { TROPICAL } from "$lib/types";
  import { fmt, loadChrome, t } from "$lib/chrome.svelte";
  import Calculator from "$lib/components/Calculator.svelte";
  import ChartCore from "$lib/components/ChartCore.svelte";
  import Beside from "$lib/components/Beside.svelte";
  import Commentary from "$lib/components/Commentary.svelte";
  import IndexOfElements from "$lib/components/IndexOfElements.svelte";
  import TopBar from "$lib/components/TopBar.svelte";
  import Wheel from "$lib/components/Wheel.svelte";
  import { formatBorn } from "$lib/civil";
  import { fade } from "svelte/transition";
  import { swapDuration } from "$lib/motion";
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
    // bar should show progress, not a stop button for a recorder that has
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
    endPresenting();
    resetFocus();
    clearNotes();
  }

  // ---- presenting ----
  //
  // The third posture: the chart shown to the client, over a call or on a
  // second screen. Everything the astrologer works with — the bar, the index,
  // the commentary, the calculation controls — is put away, and the plate
  // takes the screen the way it did in the "unbounded chart" spike this grew
  // out of. What survives is what pointing at a chart needs: click an element
  // and it lights, the hub reads it out, and a line can be written across the
  // foot of the sky without leaving the screen the client is watching. A take
  // that is running keeps running, and its stop control comes along.
  let fullscreen = $state(false);
  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;

  // The plate does not move house to be presented. The reading's layout stays
  // exactly as it is underneath; the plate is carried to the middle of the
  // window and grown there by one transform, and everything around it fades
  // and drifts aside. Coming back is the same transform let go. Swapping the
  // layout instead — unmounting the bar, collapsing the columns, resizing the
  // figure — was what made the move skip: none of that animates, so the plate
  // jumped to wherever the new layout put it.
  let plateBox = $state<HTMLElement | null>(null);
  let carry = $state<string | null>(null);

  /** The transform that carries the plate from where the reading lays it out
   * to the centre of the window, at presenting size. Measured from the plate's
   * layout box (its `offset*` sizes ignore the transform itself), so it can be
   * re-measured mid-presentation without compounding. */
  function fit() {
    if (!isPresenting() || !plateBox) {
      carry = null;
      return;
    }
    const fig = plateBox.offsetParent as HTMLElement | null;
    if (!fig) return;
    const f = fig.getBoundingClientRect();
    const w = plateBox.offsetWidth;
    const h = plateBox.offsetHeight;
    const cx = f.left + plateBox.offsetLeft + w / 2;
    const cy = f.top + plateBox.offsetTop + h / 2;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const size = Math.min(vh - 112, vw * 0.88, 1100);
    const k = size / w;
    carry = `translate(${vw / 2 - cx}px, ${vh / 2 - cy}px) scale(${k})`;
  }

  // While presenting the page holds still, so the plate's measured place is
  // the place it stays. The scrollbar's width is given back as padding, so
  // hiding it does not shift the layout under the plate.
  function holdScroll(on: boolean) {
    const root = document.documentElement;
    if (on) {
      root.style.paddingRight = `${window.innerWidth - root.clientWidth}px`;
      root.style.overflow = "hidden";
    } else {
      root.style.overflow = "";
      root.style.paddingRight = "";
    }
  }

  function startPresenting() {
    clearPins();
    holdScroll(true);
    setPresenting(true);
    fit();
    wake();
  }
  async function endPresenting() {
    if (!isPresenting()) return;
    setPresenting(false);
    carry = null;
    clearPins();
    clearTimeout(idleTimer);
    idle = false;
    holdScroll(false);
    if (fullscreen) await toggleFullscreen();
  }

  // Re-measure when the room changes under a presentation — the window
  // resized, full screen toggled, or a take starting widens the plate's
  // column behind it.
  $effect(() => {
    if (!plateBox) return;
    const ro = new ResizeObserver(() => fit());
    ro.observe(plateBox);
    const onResize = () => fit();
    window.addEventListener("resize", onResize);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", onResize);
    };
  });
  async function toggleFullscreen() {
    try {
      await getCurrentWindow().setFullscreen(!fullscreen);
      fullscreen = !fullscreen;
    } catch (e) {
      notify(reason(e), "error");
    }
  }
  // The controls are for the astrologer, not the client: they fade once the
  // pointer rests, and come back the moment it moves.
  function wake() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => (idle = true), 2600);
  }

  // A line written while presenting, filed under what is lit on the plate.
  let annotation = $state("");
  let keeping = $state(false);
  const lit = $derived(pinnedTags());
  const litNames = $derived.by(() => {
    if (!reading) return "";
    const names = new Map(elementsOf(reading).map((e) => [e.tag, e.name]));
    return lit.map((t) => names.get(t) ?? t).join(" · ");
  });
  async function keepAnnotation() {
    const words = annotation.trim();
    if (!words) return;
    keeping = true;
    try {
      setNotes(await writeNote(words, lit));
      annotation = "";
      clearPins();
      notify("note kept");
    } catch (e) {
      notify(reason(e), "error");
    } finally {
      keeping = false;
    }
  }
  // The click that lit an element left focus on it, where a typed space would
  // toggle it straight back off — so the line takes the caret as it opens.
  function takeFocus(node: HTMLTextAreaElement) {
    queueMicrotask(() => node.focus());
  }
  function dismissAnnotation() {
    annotation = "";
    clearPins();
  }

  function onPresentKey(e: KeyboardEvent) {
    if (!isPresenting()) return;
    wake();
    const t = e.target as HTMLElement | null;
    const typing = t?.tagName === "TEXTAREA" || t?.tagName === "INPUT";
    if (e.key === "Escape") {
      if (typing || pinCount() > 0) dismissAnnotation();
      else endPresenting();
    } else if (!typing && (e.key === "f" || e.key === "F")) {
      toggleFullscreen();
    }
  }
</script>

<svelte:window onkeydown={onPresentKey} onmousemove={() => isPresenting() && wake()} />

{#if reading}
  <!-- Whose reading this is, said once, at the head of the screen; and the
       two things you do with a reading — speak into it, send it — set where
       the eye starts rather than on a rail along the foot. -->
  <TopBar away={isPresenting()}>
    {#snippet lead()}
      <button
        type="button"
        class="btn quiet icon"
        onclick={back}
        disabled={whyCannotLeave() !== null}
        title={whyCannotLeave() ?? "back to the sky"}
        aria-label="new reading"
      >
        <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M10 3.5 5.5 8l4.5 4.5" /></svg>
      </button>
      <div class="who">
        <h1>{reading.meta.name}</h1>
        <p class="vitals">
          {formatBorn(reading.meta.born, reading.meta.locale ?? "en")}{reading.meta.place
            ? ` · ${reading.meta.place}`
            : ""}
        </p>
      </div>
    {/snippet}
    {#snippet acts()}
      <button type="button" class="btn quiet" onclick={startPresenting} title="show the chart to your client — just the plate">
        <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
          ><rect x="1.8" y="2.6" width="12.4" height="8.4" rx="1.2" /><path d="M8 11v2.4M5.2 13.6h5.6" /></svg
        >
        Present
      </button>
      {#if canRecord()}
        <button
          type="button"
          class="btn quiet rec"
          class:on={isRecording()}
          onclick={toggleRecording}
          disabled={!isRecording() && (isBusy() || whyCannotRecord() !== null)}
        >
          {#if isRecording()}
            <span class="dot" aria-hidden="true"></span> Stop · <span class="tnum">{mmss(recSecs)}</span>
          {:else if phase.kind === "transcribe"}
            Transcribing… <span class="tnum">{phase.pct}%</span>
          {:else}
            <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
              ><rect x="5.5" y="1.8" width="5" height="8.4" rx="2.5" /><path d="M3.2 7.6a4.8 4.8 0 0 0 9.6 0M8 12.4v2" /></svg
            >
            Transcribe
          {/if}
        </button>
      {/if}
      <span class="export" role="group" aria-label="export">
        <button
          type="button"
          class="btn primary"
          onclick={engrave}
          disabled={isRecording()}
          title="the self-contained HTML reading — opens in any browser"
        >
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
            ><path d="M8 2v8M4.8 6.8 8 10l3.2-3.2M2.8 12.2v1.3h10.4v-1.3" /></svg
          >
          Export reading
        </button>
        <button
          type="button"
          class="btn quiet"
          onclick={engravePdf}
          disabled={isRecording()}
          title="a printer-friendly PDF">PDF</button
        >
      </span>
    {/snippet}
  </TopBar>

  <div
    class="reading"
    class:presenting={isPresenting()}
    data-posture={isRecording() ? "recording" : "curating"}
  >
    <figure class="plate">
      <!-- the wheel stands on the sky, unframed; this box only anchors the hub
           read-out, which is absolutely centred over it -->
      <div class="plate-box" bind:this={plateBox} style:transform={carry}>
        <!-- keyed on the calculation so a live reproject remounts the wheel and
             replays its ring-draw/rise-in entrance as the transition; ChartCore
             stays mounted so the caption control keeps focus across the swap -->
        {#key reading.meta.house_system + "|" + (reading.meta.ayanamsa ?? TROPICAL)}
          <Wheel chart={reading} notes={notes()} />
        {/key}
        <ChartCore chart={reading} notes={notes()} bare={isPresenting()} />
      </div>
      <!-- the index is the plate's legend, not a region competing with the
           commentary: it belongs to the figure it annotates, the way a key
           sits under an engraved plate rather than beside it -->
      <figcaption class="legend">
        <IndexOfElements chart={reading} />
      </figcaption>
    </figure>

    <section class="stream">
      <!-- The filter, as one sentence with its switch in it: "notes touching
           [any | all] of the selection", the count at the far end. -->
      <div class="station">
        <span class="phrase">{t().notesTouching}</span>
        <span class="seg" role="group" aria-label="match">
          <button
            type="button"
            aria-pressed={mode() === "any"}
            onclick={() => setMode("any")}
            title={t().anyTitle}>{t().any}</button
          ><button
            type="button"
            aria-pressed={mode() === "all"}
            onclick={() => setMode("all")}
            title={t().allTitle}>{t().all}</button
          >
        </span>
        <span class="phrase">{t().ofSelection.replace(/\s*·\s*$/, "")}</span>
        {#if pinCount() > 0}
          <button type="button" class="btn bare small" onclick={clearPins}>{t().clear}</button>
        {/if}
        <span class="count">{fmt(t().count, { shown: visible.length, total: notes().length })}</span>
      </div>

      <Commentary chart={reading} {visible} />

      <Beside chart={reading} />
    </section>
  </div>

  {#if isPresenting()}
    <!-- What the client sees named: whose sky this is. -->
    <header class="present-title" in:fade={{ duration: swapDuration() ? 420 : 0, delay: swapDuration() ? 260 : 0 }} out:fade={{ duration: swapDuration() ? 160 : 0 }}>
      <h1>{reading.meta.name}</h1>
      <p>
        {formatBorn(reading.meta.born, reading.meta.locale ?? "en")}{reading.meta.place
          ? ` · ${reading.meta.place}`
          : ""}
      </p>
    </header>

    <!-- The astrologer's own controls, out of the way and fading at rest. -->
    <div
      class="present-controls"
      class:idle={idle && !isRecording()}
      in:fade={{ duration: swapDuration() ? 420 : 0, delay: swapDuration() ? 260 : 0 }}
      out:fade={{ duration: swapDuration() ? 160 : 0 }}
    >
      {#if isRecording()}
        <button type="button" class="btn quiet rec on" onclick={toggleRecording}>
          <span class="dot" aria-hidden="true"></span> Stop · <span class="tnum">{mmss(recSecs)}</span>
        </button>
      {/if}
      <button
        type="button"
        class="btn quiet icon"
        onclick={toggleFullscreen}
        title={fullscreen ? "leave full screen (F)" : "full screen (F)"}
        aria-label={fullscreen ? "leave full screen" : "full screen"}
        aria-pressed={fullscreen}
      >
        <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
          {#if fullscreen}
            <path d="M6 2.5V6H2.5M10 2.5V6h3.5M6 13.5V10H2.5M10 13.5V10h3.5" />
          {:else}
            <path d="M2.5 6V2.5H6M13.5 6V2.5H10M2.5 10v3.5H6M13.5 10v3.5H10" />
          {/if}
        </svg>
      </button>
      <button type="button" class="btn quiet" onclick={endPresenting} title="back to the reading (Esc)">
        End presenting
      </button>
    </div>

    <!-- Written over the foot of the sky: the plate keeps its size and centre
         under the hand that writes on it, the way a print stays put. Nothing
         is reserved for it — with nothing lit, the screen is only sky. -->
    {#if lit.length > 0}
      <div class="annotation">
        <p class="filed">{litNames}</p>
        <textarea
          bind:value={annotation}
          rows="2"
          use:takeFocus
          placeholder="What you see here — kept with the reading"
          aria-label="a note on {litNames}"
          onkeydown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) keepAnnotation();
          }}
        ></textarea>
        <p class="annotation-acts">
          <span class="hint">⌘↩ to keep · Esc to put away</span>
          <button type="button" class="btn bare small" onclick={dismissAnnotation}>Put away</button>
          <button
            type="button"
            class="btn primary small"
            disabled={keeping || annotation.trim() === ""}
            onclick={keepAnnotation}>{keeping ? "Keeping…" : "Keep"}</button
          >
        </p>
      </div>
    {/if}
  {/if}
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
    gap: clamp(1.6rem, 3.5vw, 3.6rem);
    padding: 0.4rem clamp(1.2rem, 3vw, 2.6rem) 3rem;
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

  /* --- presenting: the plate carried forward, the rest stepping aside ---
     Nothing here changes the layout. The plate's own transform (set from
     `fit`) carries it; everything else only fades and drifts, so the page
     underneath is the same page when the presentation ends. */
  .plate-box {
    transform-origin: 50% 50%;
    transition: transform 720ms var(--ease-out-expo);
  }
  .reading.presenting .plate {
    z-index: var(--z-chrome);
  }
  .reading .stream,
  .reading .legend {
    transition:
      opacity 420ms var(--ease-out-quint),
      transform 720ms var(--ease-out-expo);
  }
  .reading.presenting .stream {
    opacity: 0;
    transform: translateX(48px);
    pointer-events: none;
  }
  .reading.presenting .legend {
    opacity: 0;
    transform: translateY(32px);
    pointer-events: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .plate-box,
    .reading .stream,
    .reading .legend {
      transition: none;
    }
  }
  .present-title {
    position: fixed;
    top: clamp(1rem, 3vh, 2rem);
    left: clamp(1.2rem, 3vw, 2.4rem);
    z-index: var(--z-chrome);
    pointer-events: none;
  }
  .present-title h1 {
    font-size: clamp(1.25rem, 2vw, 1.6rem);
  }
  .present-title p {
    margin: 0.2rem 0 0;
    font-family: var(--font-sans);
    font-size: 0.82rem;
    color: var(--ink-3);
  }
  .present-controls {
    position: fixed;
    top: clamp(1rem, 3vh, 2rem);
    right: clamp(1.2rem, 3vw, 2.4rem);
    z-index: var(--z-chrome);
    display: flex;
    gap: 0.4rem;
    transition: opacity var(--dur-slow) var(--ease-out-expo);
  }
  .present-controls.idle {
    opacity: 0;
  }
  .present-controls.idle:focus-within {
    opacity: 1;
  }
  .annotation {
    position: fixed;
    bottom: clamp(1.2rem, 4vh, 2.6rem);
    left: 50%;
    z-index: var(--z-chrome);
    transform: translateX(-50%);
    width: min(92vw, 44rem);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.9rem 1rem 0.75rem;
    border-radius: 12px;
    background: rgba(26, 31, 69, 0.92);
    -webkit-backdrop-filter: blur(12px);
    backdrop-filter: blur(12px);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 24px 60px -18px rgba(0, 0, 0, 0.75);
    font-family: var(--font-sans);
  }
  .filed {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--brass);
  }
  .annotation textarea {
    width: 100%;
    padding: 0.2rem 0;
    background: transparent;
    border: none;
    color: var(--ink);
    font-family: var(--font-serif);
    font-size: 1.05rem;
    line-height: 1.6;
    resize: none;
  }
  .annotation textarea:focus {
    outline: none;
  }
  .annotation textarea::placeholder {
    font-style: normal;
    color: var(--ink-3);
  }
  .annotation-acts {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    padding-top: 0.55rem;
    box-shadow: 0 -1px 0 var(--line);
  }
  .annotation-acts .hint {
    margin-right: auto;
    font-size: 0.74rem;
    color: var(--ink-3);
  }
  @media (prefers-reduced-motion: no-preference) {
    .annotation {
      animation: rise var(--dur-base) var(--ease-out-expo);
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translate(-50%, 6px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .present-controls {
      transition: none;
    }
  }

  /* --- the head of the screen --- */
  .who {
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 1.15rem;
    font-weight: 400;
    line-height: 1.2;
    color: var(--ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .vitals {
    margin: 0.1rem 0 0;
    font-size: 0.78rem;
    color: var(--ink-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .export {
    display: inline-flex;
    gap: 0.35rem;
  }
  .tnum {
    font-variant-numeric: tabular-nums;
  }
  .rec.on {
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--oxblood);
    background: var(--oxblood-wash);
  }
  .dot {
    display: inline-block;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--oxblood);
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
      opacity: 0.3;
    }
    100% {
      opacity: 1;
    }
  }

  /* --- the plate --- */
  .plate {
    margin: 0;
    position: sticky;
    top: calc(var(--bar-h) + 0.4rem);
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 1.4rem;
  }
  .plate-box {
    position: relative;
  }
  .legend {
    padding: 0 0.2rem;
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

  /* --- the stream's filter line --- */
  .station {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin: 0 0 0.6rem;
    padding: 0.5rem 0 0.7rem;
    border-bottom: 1px solid var(--line);
    font-family: var(--font-sans);
    font-size: 0.84rem;
  }
  .phrase {
    color: var(--ink-3);
  }
  /* any / all as one two-way switch */
  .seg {
    display: inline-flex;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .seg button {
    padding: 0.2rem 0.65rem;
    border-radius: 3px;
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--ink-3);
    transition:
      background var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  .seg button:hover {
    color: var(--ink);
  }
  .seg button[aria-pressed="true"] {
    background: var(--surface-raised);
    color: var(--ink);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35), inset 0 0 0 1px var(--hairline);
  }
  .seg button:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 1px;
  }
  .count {
    margin-left: auto;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  @media (prefers-reduced-motion: reduce) {
    .seg button {
      transition: none;
    }
  }
</style>
