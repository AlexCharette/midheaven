<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { onTranscribeProgress, saveArtifact, startRecording, stopRecording } from "$lib/api";
  import { app, selected, visibleExcerpts } from "$lib/state.svelte";
  import BirthForm from "$lib/components/BirthForm.svelte";
  import Commentary from "$lib/components/Commentary.svelte";
  import IndexOfElements from "$lib/components/IndexOfElements.svelte";
  import Wheel from "$lib/components/Wheel.svelte";
  import { onMount } from "svelte";

  const visible = $derived(app.chart ? visibleExcerpts(app.chart) : []);

  // transcription progress can arrive during a form build or a live take
  onMount(() => {
    const unlisten = onTranscribeProgress((pct) => {
      if (app.busy !== false) app.busy = pct;
    });
    return () => {
      unlisten.then((f) => f());
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
        app.status = "listening — speak the reading; stop to route it";
      } catch (e) {
        app.status = `✗ ${e}`;
      }
      return;
    }
    clearInterval(recTimer);
    recording = false;
    app.busy = "compute";
    app.status = "routing the recording…";
    try {
      app.chart = await stopRecording();
      app.status = `${app.chart.excerpts.length} passages on the chart`;
    } catch (e) {
      app.status = `✗ ${e}`;
    } finally {
      app.busy = false;
    }
  }

  async function engrave() {
    const path = await save({
      defaultPath: "reading.html",
      filters: [{ name: "HTML artifact", extensions: ["html"] }],
    });
    if (!path) return;
    try {
      const written = await saveArtifact(path);
      app.status = `wrote ${written} ☞ open it in a browser`;
    } catch (e) {
      app.status = `✗ ${e}`;
    }
  }

  function back() {
    app.chart = null;
    app.status = "";
    selected.clear();
  }
</script>

{#if app.chart}
  <div class="reading">
    <figure class="plate">
      <div class="plate-frame">
        <Wheel chart={app.chart} />
      </div>
    </figure>

    <section>
      <IndexOfElements chart={app.chart} />

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

      <Commentary chart={app.chart} {visible} />
    </section>
  </div>
  <footer>
    <span class="apparatus-text status">{app.status}</span>
    <span class="foot-actions">
      <button class="ghost" onclick={back}>← new reading</button>
      {#if app.model}
        <button
          class="frame-btn rec"
          class:on={recording}
          onclick={toggleRecording}
          disabled={!recording && app.busy !== false}
        >
          {#if recording}
            <span class="dot" aria-hidden="true"></span> stop transcribing · {mmss(recSecs)}
          {:else if typeof app.busy === "number"}
            transcribing… {app.busy}%
          {:else}
            ◉ begin transcribing
          {/if}
        </button>
      {/if}
      <button class="frame-btn" onclick={engrave} disabled={recording}>engrave the artifact</button>
    </span>
  </footer>
{:else}
  <BirthForm />
  {#if app.status}<footer><span class="apparatus-text status">{app.status}</span></footer>{/if}
{/if}

<style>
  .reading {
    display: grid;
    grid-template-columns: minmax(360px, 44%) 1fr;
    gap: 2.2rem;
    padding: 1.6rem 1.8rem 3.4rem;
    max-width: 1400px;
    margin: 0 auto;
  }
  .plate {
    margin: 0;
    position: sticky;
    top: 1.2rem;
    align-self: start;
  }
  .plate-frame {
    border: 1px solid var(--hairline);
    outline: 1px solid var(--line);
    outline-offset: 5px;
    padding: 0.8rem;
    margin: 6px;
    background: radial-gradient(ellipse at 50% 42%, rgba(27, 32, 73, 0.6) 0%, transparent 70%);
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
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.35rem 1.2rem;
    background: var(--bg-deep);
    border-top: 1px solid var(--line);
    font-size: 0.88rem;
  }
  .foot-actions {
    margin-left: auto;
    display: inline-flex;
    gap: 1.4rem;
  }
  .status {
    min-height: 1.2em;
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
