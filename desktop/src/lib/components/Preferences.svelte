<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { getPreferences, listModels, openLicenses, setPreferences } from "$lib/api";
  import { ayanamsas, defaults, houseSystems, locales } from "$lib/options.svelte";
  import { SIDEREAL, TROPICAL } from "$lib/types";
  import { notify } from "$lib/toasts.svelte";
  import { basename } from "$lib/files";
  import { reason } from "$lib/failure";

  let { onclose }: { onclose: () => void } = $props();

  let readingsDir = $state("");
  let modelsDir = $state("");
  let defaultModel = $state("");
  let astrologer = $state("");
  let logo = $state("");
  let pageSize = $state("a4");
  let defaultLocale = $state("en");
  let defaultHouseSystem = $state(defaults().houseSystem);
  let defaultZodiac = $state(defaults().zodiac);
  let defaultAyanamsa = $state(defaults().ayanamsa);
  let models = $state<string[]>([]);
  let error = $state("");
  // Not a pane field — the calculator's remembered place, carried through the
  // save so keeping preferences never erases it.
  let lastPlaceId = $state<number | null>(null);


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
      defaultLocale = p.default_locale ?? "en";
      defaultHouseSystem = p.default_house_system ?? defaults().houseSystem;
      defaultZodiac = p.default_zodiac ?? defaults().zodiac;
      defaultAyanamsa = p.default_ayanamsa ?? defaults().ayanamsa;
      lastPlaceId = p.last_place_id;
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

  async function showLicenses() {
    try {
      await openLicenses();
    } catch (e) {
      error = reason(e);
    }
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
        default_locale: defaultLocale === "en" ? null : defaultLocale,
        // A choice equal to the default is stored as absent, so the default
        // can change later without every saved preference pinning the old one.
        default_house_system:
          defaultHouseSystem === defaults().houseSystem ? null : defaultHouseSystem,
        default_zodiac: defaultZodiac === defaults().zodiac ? null : defaultZodiac,
        default_ayanamsa: defaultAyanamsa === defaults().ayanamsa ? null : defaultAyanamsa,
        last_place_id: lastPlaceId,
      });
      notify("preferences kept");
      onclose();
    } catch (e) {
      error = reason(e);
    }
  }
</script>

<p class="rubric">preferences</p>
<form
  onsubmit={(e) => {
    e.preventDefault();
    keep();
  }}
>
  <p class="section">library</p>
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

  <p class="section">identity</p>
  <label>
    <span>your name</span>
    <input bind:value={astrologer} placeholder="artifacts read “prepared by …” (optional)" />
  </label>
  <label>
    <span>your logo</span>
    <input bind:value={logo} placeholder="engraved on the title plate (optional)" />
    <button type="button" class="browse" onclick={pickLogo}>browse…</button>
  </label>

  <p class="section">output</p>
  <label>
    <span>paper size</span>
    <select bind:value={pageSize}>
      <option value="a4">A4</option>
      <option value="letter">US Letter</option>
    </select>
  </label>
  <label>
    <span>default language</span>
    <select bind:value={defaultLocale}>
      {#each locales as l (l.code)}
        <option value={l.code}>{l.label}</option>
      {/each}
    </select>
  </label>

  <p class="section">calculation</p>
  <label>
    <span>house system</span>
    <select bind:value={defaultHouseSystem}>
      {#each houseSystems as h (h.code)}
        <option value={h.code}>{h.label}</option>
      {/each}
    </select>
  </label>
  <label>
    <span>zodiac</span>
    <select bind:value={defaultZodiac}>
      <option value={TROPICAL}>Tropical</option>
      <option value={SIDEREAL}>Sidereal</option>
    </select>
  </label>
  {#if defaultZodiac === SIDEREAL}
    <label>
      <span>ayanamsa</span>
      <select bind:value={defaultAyanamsa}>
        {#each ayanamsas as a (a.code)}
          <option value={a.code}>{a.label}</option>
        {/each}
      </select>
    </label>
  {/if}

  <p class="section">about</p>
  <label>
    <span>acknowledgements</span>
    <span class="ack">
      Ephemeris and house calculations by the
      <button type="button" class="linklike" onclick={showLicenses}>xalen</button>
      crates (Apache-2.0).
    </span>
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
    margin-top: 1.6rem;
    text-align: left;
  }
  /* group headings: small-caps section labels, each opening a run of fields */
  .section {
    font-variant: small-caps;
    letter-spacing: 0.16em;
    color: var(--ink-3);
    margin: 0 0 0.9rem;
    padding-bottom: 0.3rem;
    border-bottom: 1px solid var(--line);
  }
  .section:not(:first-of-type) {
    margin-top: 1.8rem;
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
  .ack {
    font-size: 0.85rem;
    color: var(--ink-3);
    line-height: 1.5;
  }
  .linklike {
    color: var(--ink);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .linklike:hover {
    color: var(--steel);
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
