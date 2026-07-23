<script lang="ts">
  import "$lib/theme.css";
  import Toaster from "$lib/components/Toaster.svelte";
  let { children } = $props();
</script>

<!-- A persistent engraved plate edge around the whole app, so every surface —
     the entry form, the library, preferences, the reading — sits inside one
     enclosure. Purely decorative: pointer-events off, drawn behind the fixed
     footer (which becomes the plate's bottom cartouche on the reading screen). -->
<div class="app-frame" aria-hidden="true">
  <span class="corner tl"></span>
  <span class="corner tr"></span>
  <span class="corner bl"></span>
  <span class="corner br"></span>
</div>

{@render children()}
<Toaster />

<style>
  .app-frame {
    position: fixed;
    inset: 9px;
    z-index: var(--z-sticky);
    border: 1px solid var(--line);
    pointer-events: none;
  }
  .corner {
    position: absolute;
    width: 13px;
    height: 13px;
    border: 1px solid var(--hairline);
  }
  .tl {
    top: -1px;
    left: -1px;
    border-right: none;
    border-bottom: none;
  }
  .tr {
    top: -1px;
    right: -1px;
    border-left: none;
    border-bottom: none;
  }
  .bl {
    bottom: -1px;
    left: -1px;
    border-right: none;
    border-top: none;
  }
  .br {
    bottom: -1px;
    right: -1px;
    border-left: none;
    border-top: none;
  }
  @media (prefers-reduced-motion: no-preference) {
    .app-frame {
      opacity: 0;
      animation: frame-in 1s var(--ease-out-quint) 0.15s forwards;
    }
  }
  @keyframes frame-in {
    to {
      opacity: 1;
    }
  }
</style>
