<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { saveArtifact } from "$lib/api";
  import { app, selected, visibleExcerpts } from "$lib/state.svelte";
  import BirthForm from "$lib/components/BirthForm.svelte";
  import Commentary from "$lib/components/Commentary.svelte";
  import IndexOfElements from "$lib/components/IndexOfElements.svelte";
  import Wheel from "$lib/components/Wheel.svelte";

  const visible = $derived(app.chart ? visibleExcerpts(app.chart) : []);

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
      <figcaption class="apparatus-text">
        <span class="fig">Fig. I.</span> — the natal figure of {app.chart.meta.name},
        {app.chart.meta.born}{app.chart.meta.place ? `, ${app.chart.meta.place}` : ""}.
        {app.chart.meta.system} houses upon the {app.chart.meta.zodiac.toLowerCase()} zodiac.
      </figcaption>
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
      <button class="frame-btn" onclick={engrave}>engrave the artifact</button>
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
  figcaption {
    margin-top: 0.9rem;
    text-align: center;
    font-size: 0.88rem;
    padding: 0 0.5rem;
  }
  figcaption .fig {
    font-style: normal;
    font-variant: small-caps;
    letter-spacing: 0.08em;
    color: var(--ink-2);
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
</style>
