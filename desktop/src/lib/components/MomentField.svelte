<script lang="ts">
  // One moment field — a native date or time picker.
  //
  // The browser owns everything it is better at than we are: the calendar, the
  // clock convention (a US machine reads 09:15 AM, a Russian one 09:15 — the
  // value is 24-hour ISO either way), segment keyboard entry, and validity. It
  // hands us a complete value or an empty string, never a malformed one, which
  // is why the hand-rolled `parse` prop and the oxblood invalid underline this
  // field used to carry are gone.
  //
  // What stays ours is the one thing native has no notion of: while the field
  // is being edited it must not be overwritten, because on the calculator a
  // ring drag moves the same instant this field is showing. An idle field
  // follows the scrub live; an `editing` one is left alone.
  let {
    value,
    type,
    id,
    width,
    oncommit,
  }: {
    /** The canonical string for the current draft instant: `YYYY-MM-DD` or `HH:MM`. */
    value: string;
    type: "date" | "time";
    /** The parent owns the visible `<label for>`; this is the other half of it. */
    id: string;
    width: string;
    /** Fires only for a complete value the browser accepted. */
    oncommit: (text: string) => void;
  } = $props();

  // Mirrors the DOM rather than driving it: `bind:value` is unavailable here
  // because Svelte forbids a dynamic `type` on a two-way-bound input, and
  // tracking every keystroke keeps a re-render from stomping a part-typed value.
  /* svelte-ignore state_referenced_locally */
  let text = $state(value);
  let editing = $state(false);

  // follow the moment while idle; never while the user is mid-thought
  $effect(() => {
    if (!editing) text = value;
  });

  const read = (e: Event) => (text = (e.currentTarget as HTMLInputElement).value);

  // `change` fires only once the segments form a value the browser accepts, so
  // an incomplete entry commits nothing and the effect reverts it on blur.
  function commit(e: Event) {
    read(e);
    if (text !== "" && text !== value) oncommit(text);
  }

  function onkeydown(e: KeyboardEvent) {
    const input = e.currentTarget as HTMLInputElement;
    if (e.key === "Enter") {
      e.preventDefault();
      input.blur();
    } else if (e.key === "Escape") {
      // Escape also closes an open picker; reverting here is what makes it mean
      // the same thing in both cases.
      text = value;
      input.blur();
    }
  }
</script>

<input
  {type}
  {id}
  value={text}
  style="width: {width}"
  oninput={read}
  onchange={commit}
  onfocus={() => (editing = true)}
  onblur={() => (editing = false)}
  {onkeydown}
/>

<style>
  input {
    font-variant-numeric: tabular-nums;
    transition: border-color var(--dur-fast) var(--ease-out-quint);
  }
  /* The picker glyph stays the platform's accent blue, and that is a decision,
     not an oversight. In WKWebView it is native chrome outside CSS reach: it
     ignores `currentColor`, it ignores `accent-color`, and a `filter` on
     `::-webkit-calendar-picker-indicator` has no effect on it either — all three
     were tried against a live build. Only its box responds.

     Two routes remain if the blue ever becomes intolerable, and both cost more
     than they look: `appearance: none` on the input, which strips the native
     control wholesale, plus our own mark calling `showPicker()` — Safari 16+, so
     it needs a capability guard for the 10.15 floor this app still supports. Do
     not spend another afternoon on a filter chain; it cannot work here. */
  input::-webkit-calendar-picker-indicator {
    margin: 0;
    padding: 0 0 0 0.35rem;
    cursor: pointer;
  }
  @media (prefers-reduced-motion: reduce) {
    input {
      transition: none;
    }
  }
</style>
