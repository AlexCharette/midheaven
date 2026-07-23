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
  import { app, excerptsMatching, isBusy, loadCalcOptions, loadLocales, notify, selected, visibleExcerpts } from "$lib/state.svelte";
  import BirthForm from "$lib/components/BirthForm.svelte";
  import ChartCore from "$lib/components/ChartCore.svelte";
  import Commentary from "$lib/components/Commentary.svelte";
  import IndexOfElements from "$lib/components/IndexOfElements.svelte";
  import Wheel from "$lib/components/Wheel.svelte";
  import { onMount } from "svelte";

  // With nothing pinned, hovering an element previews just its passages;
  // once anything is pinned the hover preview stops and the commentary tracks
  // the pinned selection (empty selection = the whole reading).
  const previewing = $derived(selected.size === 0 && app.hovered !== null);
  const visible = $derived(
    app.chart
      ? previewing
        ? excerptsMatching(app.chart, [app.hovered!], "any")
        : visibleExcerpts(app.chart)
      : [],
  );

  // transcription progress can arrive during a form build or a live take
  onMount(() => {
    // The reading-language list (endonyms + house suffixes) comes from the
    // backend once; the form and preferences selectors read it from state.
    loadLocales();
    loadCalcOptions();
    // A configured default model enables live transcription on ANY open chart,
    // not only ones just built through the form (which sets app.model itself) —
    // so a reading opened from the library can still be transcribed onto.
    if (!app.model) {
      getPreferences()
        .then((p) => {
          if (!app.model && p.default_model) app.model = p.default_model;
        })
        .catch(() => {});
    }
    const unlisten = onTranscribeProgress((pct) => {
      if (isBusy()) app.busy = { kind: "transcribe", pct };
    });
    // Warnings the pipeline used to write to stderr now surface as toasts.
    const unlistenWarn = onBuildWarnings((ws) => ws.forEach((w) => notify(w)));
    return () => {
      unlisten.then((f) => f());
      unlistenWarn.then((f) => f());
    };
  });

  // ---- live session recording ----
  let recording = $state(false);
  let recSecs = $state(0);
  let recTimer: ReturnType<typeof setInterval> | undefined;
  const mmss = (s: number) =>
    `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;

  async function toggleRecording() {
    if (!recording) {
      try {
        await startRecording(app.model);
        recording = true;
        recSecs = 0;
        recTimer = setInterval(() => recSecs++, 1000);
        notify("listening — speak the reading; stop to route it");
      } catch (e) {
        notify(`${e}`, "error");
      }
      return;
    }
    clearInterval(recTimer);
    recording = false;
    app.busy = { kind: "compute" };
    notify("routing the recording…");
    try {
      app.chart = await stopRecording();
      notify(`${app.chart.excerpts.length} passages on the chart`);
    } catch (e) {
      notify(`${e}`, "error");
    } finally {
      app.busy = { kind: "idle" };
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
      notify(`${e}`, "error");
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
      notify(`${e}`, "error");
    }
  }

  function back() {
    app.chart = null;
    app.hovered = null;
    selected.clear();
  }
</script>

{#if app.chart}
  <div class="reading">
    <figure class="plate">
      <div class="plate-frame">
        <Wheel chart={app.chart} />
        <ChartCore chart={app.chart} />
      </div>
    </figure>

    <section>
      <div class="toolbar">
        <span class="apparatus-text">passages touching</span>
        <span class="segmented">
          <button aria-pressed={app.mode === "any"} onclick={() => (app.mode = "any")}>any</button>
          <button aria-pressed={app.mode === "all"} onclick={() => (app.mode = "all")}>all</button>
        </span>
        <span class="apparatus-text">of the selection ·</span>
        <button class="ghost" onclick={() => selected.clear()}>clear</button>
        <span class="count apparatus-text">{visible.length} of {app.chart.excerpts.length} passages</span>
      </div>

      <IndexOfElements chart={app.chart} />

      <Commentary chart={app.chart} {visible} />
    </section>
  </div>
  <footer>
    <button class="ghost" onclick={back}>← new reading</button>
    <span class="foot-actions">
      {#if app.model}
        <button
          class="frame-btn rec"
          class:on={recording}
          onclick={toggleRecording}
          disabled={!recording && isBusy()}
        >
          {#if recording}
            <span class="dot" aria-hidden="true"></span> stop transcribing · {mmss(recSecs)}
          {:else if app.busy.kind === "transcribe"}
            transcribing… {app.busy.pct}%
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
          disabled={recording}
          title="the self-contained HTML reading — opens in any browser"
        >HTML</button>
        <button
          class="frame-btn"
          onclick={engravePdf}
          disabled={recording}
          title="a printer-friendly PDF"
        >PDF</button>
      </span>
    </span>
  </footer>
{:else}
  <BirthForm />
{/if}

<style>
  /* the wheel is the hero plate; the apparatus and commentary are its caption */
  .reading {
    display: grid;
    grid-template-columns: minmax(520px, 60%) 1fr;
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
