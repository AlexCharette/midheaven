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

<form
  class="form"
  onsubmit={(e) => {
    e.preventDefault();
    keep();
  }}
>
  <h3 class="group-head">Library</h3>
  <label class="fld">
    <span class="lbl">Readings folder <span class="opt">— charts save themselves here</span></span>
    <span class="with-act">
      <input bind:value={readingsDir} placeholder="None chosen" />
      <button type="button" class="btn quiet" onclick={() => pickDir("readings")}>Browse…</button>
    </span>
  </label>
  <label class="fld">
    <span class="lbl">Models folder <span class="opt">— ggml whisper models</span></span>
    <span class="with-act">
      <input bind:value={modelsDir} onchange={refreshModels} placeholder="None chosen" />
      <button type="button" class="btn quiet" onclick={() => pickDir("models")}>Browse…</button>
    </span>
  </label>
  <label class="fld">
    <span class="lbl">Default model</span>
    <select bind:value={defaultModel} disabled={models.length === 0}>
      <option value="">None</option>
      {#each models as m (m)}
        <option value={m}>{basename(m)}</option>
      {/each}
    </select>
  </label>

  <h3 class="group-head">Identity</h3>
  <label class="fld">
    <span class="lbl">Your name <span class="opt">— artifacts read “prepared by …”</span></span>
    <input bind:value={astrologer} placeholder="Optional" />
  </label>
  <label class="fld">
    <span class="lbl">Your logo <span class="opt">— engraved on the title plate</span></span>
    <span class="with-act">
      <input bind:value={logo} placeholder="Optional" />
      <button type="button" class="btn quiet" onclick={pickLogo}>Browse…</button>
    </span>
  </label>

  <h3 class="group-head">Output</h3>
  <div class="row">
    <label class="fld">
      <span class="lbl">Paper size</span>
      <select bind:value={pageSize}>
        <option value="a4">A4</option>
        <option value="letter">US Letter</option>
      </select>
    </label>
    <label class="fld">
      <span class="lbl">Default language</span>
      <select bind:value={defaultLocale}>
        {#each locales as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
  </div>

  <h3 class="group-head">Calculation</h3>
  <div class="row">
    <label class="fld">
      <span class="lbl">House system</span>
      <select bind:value={defaultHouseSystem}>
        {#each houseSystems as h (h.code)}
          <option value={h.code}>{h.label}</option>
        {/each}
      </select>
    </label>
    <label class="fld">
      <span class="lbl">Zodiac</span>
      <select bind:value={defaultZodiac}>
        <option value={TROPICAL}>Tropical</option>
        <option value={SIDEREAL}>Sidereal</option>
      </select>
    </label>
    {#if defaultZodiac === SIDEREAL}
      <label class="fld">
        <span class="lbl">Ayanamsa</span>
        <select bind:value={defaultAyanamsa}>
          {#each ayanamsas as a (a.code)}
            <option value={a.code}>{a.label}</option>
          {/each}
        </select>
      </label>
    {/if}
  </div>

  <h3 class="group-head">About</h3>
  <!-- Not a <label>: nothing here is a field. It used to be one, and since a
       <button> is a labelable element the label bound itself to the licenses
       button — clicking anywhere in this sentence opened the licenses file. -->
  <p class="ack">
    Ephemeris and house calculations by the
    <button type="button" class="linklike" onclick={showLicenses}>xalen</button>
    crates (Apache-2.0).
  </p>

  <!-- the sheet's foot stays in reach however far the form scrolls -->
  <div class="actions">
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <span class="grow"></span>
    <button type="button" class="btn bare" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary">Save preferences</button>
  </div>
</form>

<style>
  .ack {
    margin: 0;
    font-size: 0.84rem;
    color: var(--ink-3);
    line-height: 1.5;
    text-wrap: pretty;
  }
  .linklike {
    color: var(--ink-2);
    text-decoration: underline;
    text-underline-offset: 3px;
    text-decoration-color: var(--hairline);
  }
  .linklike:hover {
    color: var(--ink);
    text-decoration-color: currentColor;
  }
  .actions {
    position: sticky;
    bottom: -1.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.6rem -1.5rem -1.5rem;
    padding: 0.9rem 1.5rem 1.1rem;
    background: var(--surface-raised);
    box-shadow: 0 -1px 0 var(--line);
  }
  .actions .error {
    flex: 1;
  }
  .grow {
    flex: 1;
  }
  .actions .error + .grow {
    display: none;
  }
</style>
