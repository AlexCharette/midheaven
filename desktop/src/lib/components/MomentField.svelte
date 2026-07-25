<script lang="ts">
  // One calculator text field (date or time): commits on Enter/blur, never
  // live-parses — an `editing` flag stops a ring drag from stomping half-typed
  // text, while an idle field follows the scrubbed moment live. Escape
  // reverts; an uncommittable value flips the underline oxblood and reverts on
  // blur.
  let {
    value,
    placeholder,
    label,
    width = "8.5rem",
    parse,
    oncommit,
  }: {
    /** The canonical string for the current draft instant. */
    value: string;
    placeholder: string;
    label: string;
    width?: string;
    /** null = not committable (shape or calendar violation). */
    parse: (text: string) => unknown | null;
    oncommit: (text: string) => void;
  } = $props();

  // one-shot capture by design: the $effect below re-syncs whenever idle
  /* svelte-ignore state_referenced_locally */
  let text = $state(value);
  let editing = $state(false);
  const invalid = $derived(editing && text.trim() !== "" && parse(text) === null);

  // follow the moment while idle; never while the user is mid-thought
  $effect(() => {
    if (!editing) text = value;
  });

  function commit() {
    if (parse(text) !== null && text !== value) oncommit(text.trim());
    else text = value;
  }
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      (e.currentTarget as HTMLInputElement).blur(); // blur commits
    } else if (e.key === "Escape") {
      text = value;
      (e.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<input
  bind:value={text}
  class:invalid
  style="width: {width}"
  {placeholder}
  aria-label={label}
  onfocus={() => (editing = true)}
  onblur={() => {
    commit();
    editing = false;
  }}
  {onkeydown}
/>

<style>
  input {
    font-variant-numeric: tabular-nums;
    transition: border-color var(--dur-fast) var(--ease-out-quint);
  }
  input.invalid {
    border-bottom-color: var(--oxblood);
  }
</style>
