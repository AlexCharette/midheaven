<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { getPreferences, listModels, setPreferences } from "$lib/api";
  import { notify } from "$lib/state.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let readingsDir = $state("");
  let modelsDir = $state("");
  let defaultModel = $state("");
  let astrologer = $state("");
  let logo = $state("");
  let pageSize = $state("a4");
  let models = $state<string[]>([]);
  let error = $state("");

  const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;

  async function refreshModels() {
    const found = modelsDir.trim() ? await listModels(modelsDir) : [];
    // a previously chosen model outside the folder stays selectable
    models = defaultModel && !found.includes(defaultModel) ? [defaultModel, ...found] : found;
  }

  $effect(() => {
    getPreferences().then((p) => {
      readingsDir = p.readings_dir ?? "";
      modelsDir = p.models_dir ?? "";
      defaultModel = p.default_model ?? "";
      astrologer = p.astrologer ?? "";
      logo = p.logo ?? "";
      pageSize = p.page_size ?? "a4";
      refreshModels();
    });
  });

  async function pickDir(kind: "readings" | "models") {
    const path = await open({ directory: true });
    if (typeof path !== "string") return;
    if (kind === "readings") {
      readingsDir = path;
    } else {
      modelsDir = path;
      await refreshModels();
      if (!models.includes(defaultModel)) defaultModel = models[0] ?? "";
    }
  }

  async function pickLogo() {
    const path = await open({
      multiple: false,
      filters: [{ name: "logo image", extensions: ["png", "jpg", "jpeg", "svg", "webp"] }],
    });
    if (typeof path === "string") logo = path;
  }

  async function keep() {
    error = "";
    try {
      await setPreferences({
        models_dir: modelsDir || null,
        default_model: defaultModel || null,
        readings_dir: readingsDir || null,
        astrologer: astrologer || null,
        logo: logo || null,
        page_size: pageSize === "a4" ? null : pageSize,
      });
      notify("preferences kept");
      onclose();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<form
  onsubmit={(e) => {
    e.preventDefault();
    keep();
  }}
>
  <label>
    <span>readings folder</span>
    <input bind:value={readingsDir} placeholder="charts save themselves here (optional)" />
    <button type="button" class="browse" onclick={() => pickDir("readings")}>browse…</button>
  </label>
  <label>
    <span>models folder</span>
    <input
      bind:value={modelsDir}
      onchange={refreshModels}
      placeholder="folder of ggml whisper models (optional)"
    />
    <button type="button" class="browse" onclick={() => pickDir("models")}>browse…</button>
  </label>
  <label>
    <span>default model</span>
    <select bind:value={defaultModel} disabled={models.length === 0}>
      <option value="">— none —</option>
      {#each models as m (m)}
        <option value={m}>{basename(m)}</option>
      {/each}
    </select>
  </label>
  <label>
    <span>your name</span>
    <input bind:value={astrologer} placeholder="artifacts read “prepared by …” (optional)" />
  </label>
  <label>
    <span>your logo</span>
    <input bind:value={logo} placeholder="engraved on the title plate (optional)" />
    <button type="button" class="browse" onclick={pickLogo}>browse…</button>
  </label>
  <label>
    <span>paper size</span>
    <select bind:value={pageSize}>
      <option value="a4">A4</option>
      <option value="letter">US Letter</option>
    </select>
  </label>

  {#if error}<p class="error">✗ {error}</p>{/if}

  <div class="actions">
    <button type="submit" class="frame-btn">keep these preferences</button>
    <button type="button" class="ghost" onclick={onclose}>cancel</button>
  </div>
</form>

<style>
  /* form grammar mirrors BirthForm: italic right-aligned labels on a
     7.5rem gutter, quiet browse buttons */
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
  }
  label span:first-child {
    font-style: italic;
    color: var(--ink-3);
    text-align: right;
  }
  select:disabled {
    color: var(--ink-3);
    font-style: italic;
  }
  .browse {
    font-size: 0.85rem;
    color: var(--ink-3);
    font-style: italic;
  }
  .browse:hover {
    color: var(--ink);
  }
  .error {
    color: var(--oxblood);
    font-style: italic;
    text-align: center;
  }
  .actions {
    text-align: center;
    margin-top: 1.6rem;
    display: flex;
    justify-content: center;
    align-items: baseline;
    gap: 1.4rem;
  }
</style>
