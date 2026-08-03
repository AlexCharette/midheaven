<script lang="ts">
  // The calculator's instrument limb: two graduated rings — time (one turn =
  // one day) and date (one turn = one year, leap-aware) — rotating under a
  // fixed brass index at 12 o'clock, an astrolabe's rete under the rule. Both
  // rings scrub the SAME civil instant at different gear ratios: dragging the
  // time ring past midnight creeps the date ring a day-tick; dragging the
  // date ring past Dec 31 rolls the year. Scales are engraved counterclock-
  // wise-increasing, so dragging clockwise advances time — and the heavens
  // (ASC-pinned) turn clockwise with the drag, rings and sky meshed like
  // gears.
  //
  // Renders its own SVG, stacked over the Wheel by Calculator: viewBox
  // -120..840 vs the Wheel's -52..772, both centred on 360 — the Wheel is
  // scaled by 824/960 in CSS so one user unit is identical in both.
  import {
    beginScrub,
    endScrub,
    minutes as draftMinutes,
    pending,
    ringPose,
    setMoment,
    snapRings,
  } from "$lib/preview.svelte";
  import {
    MINUTES_PER_DAY,
    MONTH_NAMES,
    dayOfYear,
    daysInMonth,
    daysInYear,
    fromDays,
    fromMinutes,
    ringAngles,
    ringDetent,
    ringDragMinutes,
    ringStep,
  } from "$lib/civil";
  import { ringDrag } from "$lib/ringDrag";

  const CX = 360;
  const CY = 360;
  // Radii continue the Wheel's R table outward from its drift ring (398).
  const R = {
    indexIn: 402,
    timeIn: 408,
    timeOut: 436,
    timeLbl: 416,
    dateIn: 440,
    dateOut: 472,
    dateLbl: 450,
    indexOut: 478,
  };
  const uid = $props.id();

  let svgEl: SVGSVGElement;

  // deg is measured CLOCKWISE from 12 o'clock — the rotation frame the rings
  // turn in. Scales are engraved clockwise-increasing (the dial convention:
  // hours and months read left-to-right around the limb), so a mark for value
  // v (angle a) sits at deg = +a and the group rotated by −a brings it under
  // the index — advancing time turns the rings counterclockwise, like winding
  // a dial forward.
  const at = (deg: number, r: number): [number, number] => {
    const rad = (deg * Math.PI) / 180;
    return [CX + r * Math.sin(rad), CY - r * Math.cos(rad)];
  };

  // ---- the drafted instant, at drag resolution ----
  // While a ring is held the rings track the pointer directly (zero latency);
  // at rest they render the pose tween, gliding in the same clock as the
  // planets. The time ring scrubs continuously (past midnight rolls the
  // date); the DATE ring steps in whole days — day-tick detents — so shifting
  // days never disturbs the time of day.
  let draft = $state<number | null>(null);
  let gestureStart = 0;
  let accum = 0; // float minutes dragged since the grab
  let held = $state<"time" | "date" | null>(null);
  let hovered = $state<"time" | "date" | null>(null);
  let focused = $state<"time" | "date" | null>(null);

  const shown = $derived(
    draft !== null
      ? ringAngles(draft)
      : ringPose(),
  );
  const live = $derived(held !== null || pending());

  // ---- scale geometry, re-derived only when the displayed year changes ----
  const year = $derived(fromDays(Math.floor((draft ?? draftMinutes()) / MINUTES_PER_DAY)).y);
  const days = $derived(daysInYear(year));

  // time ring: 5-minute graduation, hour ticks longest, 15-min mid
  const timeTicks = $derived(
    Array.from({ length: 288 }, (_, i) => {
      const deg = (i * 5 * 360) / MINUTES_PER_DAY;
      const [len, w] = i % 12 === 0 ? [11, 0.9] : i % 3 === 0 ? [7, 0.7] : [4.5, 0.45];
      const [x1, y1] = at(deg, R.timeOut);
      const [x2, y2] = at(deg, R.timeOut - len);
      return { x1, y1, x2, y2, w };
    }),
  );
  // hour numerals every 2h, the cardinal watches (00 06 12 18) set larger
  const hourLabels = $derived(
    Array.from({ length: 12 }, (_, i) => {
      const h = i * 2;
      const deg = h * 15;
      const [x, y] = at(deg, R.timeLbl);
      return { h: String(h).padStart(2, "0"), x, y, deg, cardinal: h % 6 === 0 };
    }),
  );

  // date ring: a day-tick per calendar day, 5th/10th days stepped, month
  // boundaries as full spokes
  const dayTicks = $derived.by(() => {
    const ticks = [];
    for (let m = 1; m <= 12; m++) {
      const dim = daysInMonth(year, m);
      for (let d = 1; d <= dim; d++) {
        if (d === 1) continue; // month boundaries get spokes instead
        const deg = ((dayOfYear(year, m, d) - 1) * 360) / days;
        const [len, w] = d % 10 === 0 ? [10, 0.9] : d % 5 === 0 ? [7, 0.7] : [4.5, 0.45];
        const [x1, y1] = at(deg, R.dateOut);
        const [x2, y2] = at(deg, R.dateOut - len);
        ticks.push({ x1, y1, x2, y2, w });
      }
    }
    return ticks;
  });
  const monthSpokes = $derived(
    Array.from({ length: 12 }, (_, i) => {
      const deg = ((dayOfYear(year, i + 1, 1) - 1) * 360) / days;
      const [x1, y1] = at(deg, R.dateIn);
      const [x2, y2] = at(deg, R.dateOut);
      return { x1, y1, x2, y2 };
    }),
  );
  // month lettering on arcs, engraved in one consistent direction (the limb
  // convention of a real instrument); the month being scrubbed sits at the
  // index, where it reads upright
  const monthArcs = $derived(
    Array.from({ length: 12 }, (_, i) => {
      const a1 = ((dayOfYear(year, i + 1, 1) - 1) * 360) / days;
      const a2 = a1 + (daysInMonth(year, i + 1) * 360) / days;
      const [x1, y1] = at(a1, R.dateLbl);
      const [x2, y2] = at(a2, R.dateLbl);
      return {
        id: `${uid}-m${i}`,
        d: `M ${x1} ${y1} A ${R.dateLbl} ${R.dateLbl} 0 0 1 ${x2} ${y2}`,
        name: MONTH_NAMES[i],
      };
    }),
  );

  // a donut-shaped hit surface (even-odd fill rule)
  const annulus = (r1: number, r2: number) =>
    `M ${CX} ${CY - r2} A ${r2} ${r2} 0 1 1 ${CX - 0.01} ${CY - r2} Z ` +
    `M ${CX} ${CY - r1} A ${r1} ${r1} 0 1 1 ${CX - 0.01} ${CY - r1} Z`;

  // ---- drag wiring: angular deltas → one unbounded float accumulator ----
  function dragHandlers(ring: "time" | "date") {
    return {
      svg: () => svgEl,
      // the ring follows the finger: rotation is −angle, so a clockwise drag
      // (positive degrees) winds the moment BACK
      degToMinutes: (deg: number) => ringDragMinutes(ring, deg, days),
      onstart: () => {
        held = ring;
        gestureStart = draftMinutes();
        accum = 0;
        draft = draftMinutes();
        beginScrub();
        // no seam at grab: park the pose's ring angles where the rings are
        snapRings(draftMinutes());
      },
      onmove: (minutes: number) => {
        if (draft === null) return;
        accum += minutes;
        draft = ringDetent(ring, gestureStart, accum);
        setMoment(draft, "drag");
      },
      onend: () => {
        if (draft !== null) setMoment(draft, "drag");
        held = null;
        draft = null;
        // release: houses and aspects, frozen through the gesture, recompute
        // and settle in with one calm glide
        endScrub();
      },
    };
  }

  // ---- keyboard: the rings as sliders (the fields stay the precision path) ----
  const minutesOfDay = $derived(((draftMinutes() % MINUTES_PER_DAY) + MINUTES_PER_DAY) % MINUTES_PER_DAY);
  const doy = $derived.by(() => {
    const { y, m, d } = fromDays(Math.floor(draftMinutes() / MINUTES_PER_DAY));
    return dayOfYear(y, m, d);
  });
  // The canonical HH:MM formatter, not a third hand-rolled zero-pad.
  const timeText = $derived(fromMinutes(draftMinutes()).time);
  const dateText = $derived.by(() => {
    const { m, d } = fromDays(Math.floor(draftMinutes() / MINUTES_PER_DAY));
    return `${d} ${MONTH_NAMES[m - 1]}`;
  });

  function onKey(ring: "time" | "date") {
    return (e: KeyboardEvent) => {
      const dir =
        e.key === "ArrowRight" || e.key === "ArrowUp" ? 1
        : e.key === "ArrowLeft" || e.key === "ArrowDown" ? -1
        : 0;
      if (dir === 0) return;
      e.preventDefault();
      setMoment(ringStep(ring, draftMinutes(), dir, e.shiftKey), "keyboard");
    };
  }
</script>

<svg bind:this={svgEl} viewBox="-120 -120 960 960" class="rings" class:live>
  <!-- edge circles are rotation-invariant, so they stay static and join the
       plate's self-draw entrance -->
  {#each [{ r: R.timeIn, d: 420, cls: "engrave", ring: "time" }, { r: R.timeOut, d: 490, cls: "engrave", ring: "time" }, { r: R.dateIn, d: 560, cls: "engrave", ring: "date" }, { r: R.dateOut, d: 630, cls: "engrave-strong", ring: "date" }] as c (c.r)}
    <circle
      cx={CX}
      cy={CY}
      r={c.r}
      pathLength="1"
      class="ring-edge {c.cls}"
      class:woken={held === c.ring || hovered === c.ring}
      style="--d: {c.d}ms"
    />
  {/each}

  <!-- TIME ring — turns once per day -->
  <g
    class="scale time-scale"
    class:held={held === "time"}
    class:hover={hovered === "time"}
    style="transform: rotate({-shown.timeAngle}deg); transform-origin: {CX}px {CY}px;"
  >
    {#each timeTicks as t, i (i)}
      <line x1={t.x1} y1={t.y1} x2={t.x2} y2={t.y2} class="tick" stroke-width={t.w} />
    {/each}
    {#each hourLabels as l (l.h)}
      <text
        x={l.x}
        y={l.y}
        class="hour"
        class:cardinal={l.cardinal}
        transform="rotate({l.deg}, {l.x}, {l.y})"
        text-anchor="middle"
        dominant-baseline="central">{l.h}</text
      >
    {/each}
  </g>

  <!-- DATE ring — the calendar limb, turns once per (leap-aware) year -->
  <g
    class="scale date-scale"
    class:held={held === "date"}
    class:hover={hovered === "date"}
    style="transform: rotate({-shown.dateAngle}deg); transform-origin: {CX}px {CY}px;"
  >
    <defs>
      {#each monthArcs as m (m.id)}
        <path id={m.id} d={m.d} fill="none" />
      {/each}
    </defs>
    {#each monthSpokes as s, i (i)}
      <line x1={s.x1} y1={s.y1} x2={s.x2} y2={s.y2} class="spoke" />
    {/each}
    {#each dayTicks as t, i (i)}
      <line x1={t.x1} y1={t.y1} x2={t.x2} y2={t.y2} class="tick" stroke-width={t.w} />
    {/each}
    {#each monthArcs as m (m.id)}
      <text class="month">
        <textPath href="#{m.id}" startOffset="50%" text-anchor="middle">{m.name}</textPath>
      </text>
    {/each}
  </g>

  <!-- the fixed brass index at 12 o'clock: the reading line both scales pass
       under; it wakes (fills solid) while the moment is in motion -->
  <g class="index" class:live aria-hidden="true">
    <line x1={CX} y1={CY - R.indexIn} x2={CX} y2={CY - R.indexOut} class="index-line" />
    <path d="M {CX} {CY - R.indexIn - 2} l -4 -8 l 8 0 z" class="index-head" />
    <circle cx={CX} cy={CY - R.indexOut} r="2.4" class="index-eye" />
  </g>

  <!-- focus marks: a dashed engraved circle at the held scale's midline (a UA
       outline won't render on an SVG group) -->
  {#if focused === "time"}
    <circle cx={CX} cy={CY} r={(R.timeIn + R.timeOut) / 2} class="focus-dash" />
  {:else if focused === "date"}
    <circle cx={CX} cy={CY} r={(R.dateIn + R.dateOut) / 2} class="focus-dash" />
  {/if}

  <!-- hit annuli: the ONLY pointer-live surfaces (the svg root is a hole, so
       the wheel keeps its own plate beneath) -->
  <path
    d={annulus(R.indexIn - 2, R.timeOut + 2)}
    fill-rule="evenodd"
    class="hit"
    role="slider"
    tabindex="0"
    aria-label="time of day"
    aria-valuemin={0}
    aria-valuemax={1439}
    aria-valuenow={minutesOfDay}
    aria-valuetext={timeText}
    aria-orientation="horizontal"
    use:ringDrag={dragHandlers("time")}
    onpointerenter={() => (hovered = "time")}
    onpointerleave={() => (hovered = null)}
    onfocus={() => (focused = "time")}
    onblur={() => (focused = null)}
    onkeydown={onKey("time")}
  />
  <path
    d={annulus(R.timeOut + 2, R.indexOut + 4)}
    fill-rule="evenodd"
    class="hit"
    role="slider"
    tabindex="0"
    aria-label="day of the year"
    aria-valuemin={1}
    aria-valuemax={days}
    aria-valuenow={doy}
    aria-valuetext={dateText}
    aria-orientation="horizontal"
    use:ringDrag={dragHandlers("date")}
    onpointerenter={() => (hovered = "date")}
    onpointerleave={() => (hovered = null)}
    onfocus={() => (focused = "date")}
    onblur={() => (focused = null)}
    onkeydown={onKey("date")}
  />
</svg>

<style>
  svg {
    width: 100%;
    height: auto;
    display: block;
    /* the rings SVG overlays the wheel: only the annuli take the pointer */
    pointer-events: none;
  }
  .engrave {
    stroke: var(--line);
    fill: none;
  }
  .engrave-strong {
    stroke: var(--hairline);
    fill: none;
  }
  .ring-edge,
  .tick,
  .spoke {
    transition: stroke var(--dur-fast) var(--ease-out-quint);
  }
  .ring-edge.woken {
    stroke: var(--hairline);
  }
  .tick,
  .spoke {
    stroke: var(--line);
  }
  .scale.hover .tick,
  .scale.held .tick,
  .scale.hover .spoke,
  .scale.held .spoke {
    stroke: var(--hairline);
  }
  .hour,
  .month {
    fill: var(--ink-3);
    transition: fill var(--dur-fast) var(--ease-out-quint);
  }
  .scale.hover .hour,
  .scale.held .hour,
  .scale.hover .month,
  .scale.held .month {
    fill: var(--ink-2);
  }
  .hour {
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.06em;
  }
  .hour.cardinal {
    font-size: 11.5px;
    fill: var(--ink-2);
  }
  .month {
    font-size: 11px;
    font-variant: small-caps;
    letter-spacing: 0.14em;
  }
  /* --- the fixed index: brass, engraved weights, no glow --- */
  .index-line {
    stroke: var(--brass);
    stroke-width: 1;
    opacity: 0.55;
    transition: opacity var(--dur-fast) var(--ease-out-quint);
  }
  .index-head {
    fill: var(--brass);
    opacity: 0.7;
    transition: opacity var(--dur-fast) var(--ease-out-quint);
  }
  .index-eye {
    fill: none;
    stroke: var(--brass);
    stroke-width: 1;
    opacity: 0.55;
    transition: opacity var(--dur-fast) var(--ease-out-quint);
  }
  .index.live .index-line,
  .index.live .index-head,
  .index.live .index-eye {
    opacity: 1;
  }
  .focus-dash {
    fill: none;
    stroke: var(--hairline);
    stroke-dasharray: 2 4;
    pointer-events: none;
  }
  .hit {
    pointer-events: auto;
    fill: transparent;
    stroke: none;
    cursor: grab;
    touch-action: none;
    outline: none;
  }
  .hit:active {
    cursor: grabbing;
  }
  /* --- entrance: the limb joins the plate's self-draw --- */
  @media (prefers-reduced-motion: no-preference) {
    .ring-edge {
      stroke-dasharray: 1;
      stroke-dashoffset: 1;
      animation: ring-draw 0.9s var(--ease-out-quint) forwards;
      animation-delay: var(--d);
    }
    .scale,
    .index {
      opacity: 0;
      animation: fade-in var(--dur-slow) var(--ease-out-quint) 0.45s forwards;
    }
  }
  @keyframes ring-draw {
    to {
      stroke-dashoffset: 0;
    }
  }
  @keyframes fade-in {
    to {
      opacity: 1;
    }
  }
</style>
