<script lang="ts">
  import "$lib/theme.css";
  import Toaster from "$lib/components/Toaster.svelte";
  let { children } = $props();
</script>

{@render children()}
<Toaster />
