<script lang="ts">
  // The calculator's moment, set as type you can turn.
  //
  // It replaces a native date picker, a native time picker and a year stepper
  // on the landing view. Those were three controls with three grammars for one
  // instant, and the platform drew two of them — calendar glyph in system blue
  // included — in a voice that belonged to no part of this app. Here the
  // moment is simply written out, large, the way the plate's caption would say
  // it, and every part of it is a spinbutton: arrow keys or the scroll wheel
  // turn it, digits type it, and the chevrons that appear on hover do the same
  // for a pointer. Stepping rolls over the way a clock does — the minute past
  // :59 carries into the hour — because that is what the rings around the
  // wheel do when you drag them, and the two should agree.
  //
  // The birth form keeps its native fields: a birth date is typed once from a
  // certificate, not explored.
  import { MINUTES_PER_DAY, MONTH_NAMES, addMonths, daysInMonth, fromDays, toDays } from "$lib/civil";
  import type { MoveSource } from "$lib/chartPose.svelte";

  let {
    minutes,
    onchange,
  }: {
    /** The draft instant in civil minutes. */
    minutes: number;
    onchange: (min: number, source: MoveSource) => void;
  } = $props();

  type Part = "day" | "month" | "year" | "hour" | "minute" | "meridiem";

  // The machine's clock convention decides whether the hour reads 20 or 8 PM —
  // the same call the native time field used to make on the user's behalf.
  const h12 = new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12 === true;

  const parts = $derived.by(() => {
    const days = Math.floor(minutes / MINUTES_PER_DAY);
    const rest = minutes - days * MINUTES_PER_DAY;
    return { ...fromDays(days), h: Math.floor(rest / 60), mi: rest % 60 };
  });

  const pad = (n: number) => String(n).padStart(2, "0");

  function shown(p: Part): string {
    const { y, m, d, h, mi } = parts;
    switch (p) {
      case "day":
        return String(d);
      case "month":
        return MONTH_NAMES[m - 1];
      case "year":
        return String(y);
      case "hour":
        return h12 ? String(h % 12 || 12) : pad(h);
      case "minute":
        return pad(mi);
      case "meridiem":
        return h < 12 ? "AM" : "PM";
    }
  }

  /** One notch of `p`, in either direction. Day, hour and minute are fixed
   * spans of civil minutes; month and year are calendar steps that clamp the
   * day (31 January + 1 month is 28 or 29 February, not 3 March). */
  function step(p: Part, delta: number, source: MoveSource) {
    const next =
      p === "day" ? minutes + delta * MINUTES_PER_DAY
      : p === "month" ? addMonths(minutes, delta)
      : p === "year" ? addMonths(minutes, delta * 12)
      : p === "hour" ? minutes + delta * 60
      : p === "minute" ? minutes + delta
      : minutes + (parts.h < 12 ? 720 : -720);
    onchange(next, source);
  }

  /** Set `p` outright from typed digits, clamping the day to its month. */
  function set(p: Part, v: number) {
    let { y, m, d, h, mi } = parts;
    if (p === "day") d = v;
    else if (p === "month") m = v;
    else if (p === "year") y = v;
    else if (p === "hour") h = h12 ? (v % 12) + (h >= 12 ? 12 : 0) : v;
    else if (p === "minute") mi = v;
    d = Math.min(d, daysInMonth(y, m));
    onchange(toDays(y, m, d) * MINUTES_PER_DAY + h * 60 + mi, "field");
  }

  // The bounds a typed value is read against, and how many digits finish it.
  const bounds: Record<Exclude<Part, "meridiem">, { min: number; max: () => number; digits: number }> = {
    day: { min: 1, max: () => daysInMonth(parts.y, parts.m), digits: 2 },
    month: { min: 1, max: () => 12, digits: 2 },
    year: { min: 1, max: () => 9999, digits: 4 },
    hour: { min: h12 ? 1 : 0, max: () => (h12 ? 12 : 23), digits: 2 },
    minute: { min: 0, max: () => 59, digits: 2 },
  };

  // Typed digits collect briefly, then land: "1" waits for a possible "2",
  // and a digit that could not start anything longer lands at once.
  let typing = $state<{ part: Part; buf: string } | null>(null);
  let typingTimer: ReturnType<typeof setTimeout> | undefined;

  function land() {
    clearTimeout(typingTimer);
    const typed = typing;
    typing = null;
    if (!typed || typed.part === "meridiem") return;
    const b = bounds[typed.part];
    const v = Number(typed.buf);
    if (v >= b.min && v <= b.max()) set(typed.part, v);
  }

  function type(p: Exclude<Part, "meridiem">, digit: string) {
    const b = bounds[p];
    const buf = (typing?.part === p ? typing.buf : "") + digit;
    typing = { part: p, buf };
    clearTimeout(typingTimer);
    if (buf.length >= b.digits || Number(buf) * 10 > b.max()) land();
    else typingTimer = setTimeout(land, 900);
  }

  function onkeydown(e: KeyboardEvent, p: Part) {
    const big = e.shiftKey ? (p === "minute" ? 15 : p === "year" ? 10 : 1) : 1;
    if (e.key === "ArrowUp") step(p, big, "keyboard");
    else if (e.key === "ArrowDown") step(p, -big, "keyboard");
    else if (e.key === "PageUp") step(p, 10, "keyboard");
    else if (e.key === "PageDown") step(p, -10, "keyboard");
    else if (p === "meridiem" && /^[ap]$/i.test(e.key)) {
      if ((e.key.toLowerCase() === "a") !== parts.h < 12) step(p, 1, "field");
    } else if (p !== "meridiem" && /^\d$/.test(e.key)) type(p, e.key);
    else if (e.key === "Enter") land();
    else if (e.key === "Escape") {
      clearTimeout(typingTimer);
      typing = null;
      return;
    } else return;
    e.preventDefault();
  }

  // A trackpad sends many small deltas; a wheel a few large ones. Accumulate
  // and step once per notch so both turn the moment at a readable rate.
  let wheelAcc = 0;
  function onwheel(e: WheelEvent, p: Part) {
    e.preventDefault();
    wheelAcc += e.deltaY;
    const notch = 36;
    while (Math.abs(wheelAcc) >= notch) {
      const dir = wheelAcc > 0 ? -1 : 1;
      wheelAcc -= -dir * notch;
      step(p, dir, "keyboard");
    }
  }

  // Wheel listeners must be non-passive to stop the page scrolling under them,
  // which Svelte's `onwheel` attribute does not promise.
  function wheel(node: HTMLElement, p: Part) {
    const h = (e: WheelEvent) => onwheel(e, p);
    node.addEventListener("wheel", h, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", h) };
  }

  const labels: Record<Part, string> = {
    day: "day",
    month: "month",
    year: "year",
    hour: "hour",
    minute: "minute",
    meridiem: "AM or PM",
  };
</script>

{#snippet seg(p: Part, cls = "")}
  <span class="seg {cls}" class:typing={typing?.part === p}>
    <button type="button" class="nudge up" tabindex="-1" aria-hidden="true" onclick={() => step(p, 1, "stepper")}>
      <svg viewBox="0 0 10 6" width="10" height="6"><path d="M1 5 5 1.2 9 5" /></svg>
    </button>
    <span
      class="val"
      role="spinbutton"
      tabindex="0"
      aria-label={labels[p]}
      aria-valuetext={shown(p)}
      onkeydown={(e) => onkeydown(e, p)}
      onblur={land}
      use:wheel={p}>{typing?.part === p ? typing.buf : shown(p)}</span
    >
    <button type="button" class="nudge down" tabindex="-1" aria-hidden="true" onclick={() => step(p, -1, "stepper")}>
      <svg viewBox="0 0 10 6" width="10" height="6"><path d="M1 1 5 4.8 9 1" /></svg>
    </button>
  </span>
{/snippet}

<div class="dial" role="group" aria-label="moment">
  <p class="date">{@render seg("day", "num")} {@render seg("month")} {@render seg("year", "num")}</p>
  <p class="time">
    {@render seg("hour", "num")}<span class="colon" aria-hidden="true">:</span>{@render seg("minute", "num")}
    {#if h12}{@render seg("meridiem", "mer")}{/if}
  </p>
</div>

<style>
  .dial {
    margin: 0;
  }
  .date,
  .time {
    margin: 0;
    font-family: var(--font-serif);
    color: var(--ink);
    line-height: 1.15;
    white-space: nowrap;
  }
  .date {
    font-size: clamp(1.45rem, 2.3vw, 1.85rem);
  }
  .time {
    margin-top: 0.15rem;
    font-size: clamp(1.45rem, 2.3vw, 1.85rem);
    color: var(--ink-2);
  }
  .colon {
    padding: 0 0.04em;
    color: var(--ink-3);
  }
  .seg {
    position: relative;
    display: inline-block;
  }
  .val {
    display: inline-block;
    padding: 0.04em 0.12em;
    margin: 0 -0.12em;
    border-radius: var(--radius-sm);
    cursor: ns-resize;
    outline: none;
    transition:
      background var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  .num .val {
    font-variant-numeric: lining-nums tabular-nums;
  }
  .mer .val {
    margin-left: 0.1em;
    font-family: var(--font-sans);
    font-size: 0.5em;
    font-weight: 500;
    letter-spacing: 0.04em;
    vertical-align: 0.5em;
    color: var(--ink-3);
  }
  .seg:hover .val {
    background: var(--surface-hover);
    color: var(--ink);
  }
  .val:focus-visible,
  .typing .val {
    background: var(--brass-wash);
    color: var(--ink);
    box-shadow: inset 0 -1px 0 var(--brass);
  }
  /* The chevrons appear with the hover, above and below the part they turn,
     so a pointer has the same two moves the keys have without the resting
     line carrying any furniture. */
  .nudge {
    position: absolute;
    left: 50%;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 1.6rem;
    height: 0.95rem;
    margin-left: -0.8rem;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-fast) var(--ease-out-quint);
  }
  .nudge svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .up {
    bottom: calc(100% - 0.1em);
  }
  .down {
    top: calc(100% - 0.1em);
  }
  .seg:hover .nudge {
    opacity: 1;
    pointer-events: auto;
  }
  .nudge:hover {
    color: var(--brass);
    background: var(--surface-hover);
  }
  @media (prefers-reduced-motion: reduce) {
    .val,
    .nudge {
      transition: none;
    }
  }
</style>
