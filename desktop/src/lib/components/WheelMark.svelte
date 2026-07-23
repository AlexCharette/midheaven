<script lang="ts">
  // A purely ornamental chart wheel — an engraved figure, no data. The hero
  // motif for the entry screen; a small sibling of the live Wheel that makes
  // the app unmistakably itself before a chart exists. Echoed on the website.
  let { size = 132 }: { size?: number } = $props();

  const pt = (deg: number, r: number): [number, number] => {
    const a = ((deg - 90) * Math.PI) / 180;
    return [50 + r * Math.cos(a), 50 + r * Math.sin(a)];
  };
  const ticks = Array.from({ length: 72 }, (_, i) => {
    const deg = i * 5;
    const long = deg % 30 === 0;
    const [x1, y1] = pt(deg, 44);
    const [x2, y2] = pt(deg, long ? 38.5 : 41.5);
    return { x1, y1, x2, y2, w: long ? 0.7 : 0.4 };
  });
  const spokes = Array.from({ length: 12 }, (_, i) => {
    const [x1, y1] = pt(i * 30, 12);
    const [x2, y2] = pt(i * 30, 30);
    return { x1, y1, x2, y2 };
  });
  const rays = Array.from({ length: 8 }, (_, i) => {
    const [x1, y1] = pt(i * 45, 1.6);
    const [x2, y2] = pt(i * 45, i % 2 === 0 ? 9 : 5.5);
    return { x1, y1, x2, y2 };
  });
</script>

<svg
  class="wheelmark"
  width={size}
  height={size}
  viewBox="0 0 100 100"
  role="img"
  aria-label="an engraved chart wheel"
>
  <circle cx="50" cy="50" r="48" pathLength="1" class="ring strong" style="--d: 0ms" />
  <circle cx="50" cy="50" r="44" pathLength="1" class="ring" style="--d: 90ms" />
  <circle cx="50" cy="50" r="30" pathLength="1" class="ring" style="--d: 180ms" />
  <circle cx="50" cy="50" r="12" pathLength="1" class="ring" style="--d: 270ms" />
  <g class="apparatus">
    {#each ticks as t, i (i)}
      <line x1={t.x1} y1={t.y1} x2={t.x2} y2={t.y2} stroke-width={t.w} class="tick" />
    {/each}
    {#each spokes as s, i (i)}
      <line x1={s.x1} y1={s.y1} x2={s.x2} y2={s.y2} class="spoke" />
    {/each}
    {#each rays as r, i (i)}
      <line x1={r.x1} y1={r.y1} x2={r.x2} y2={r.y2} class="ray" />
    {/each}
    <circle cx="50" cy="50" r="1.3" class="hub" />
  </g>
</svg>

<style>
  .wheelmark {
    display: block;
    overflow: visible;
  }
  .ring {
    fill: none;
    stroke: var(--line);
  }
  .ring.strong {
    stroke: var(--hairline);
  }
  .tick,
  .spoke {
    stroke: var(--line);
  }
  .ray {
    stroke: var(--hairline);
    stroke-width: 0.5;
  }
  .hub {
    fill: var(--brass);
  }
  @media (prefers-reduced-motion: no-preference) {
    .ring {
      stroke-dasharray: 1;
      stroke-dashoffset: 1;
      animation: mark-draw 0.9s var(--ease-out-quint) forwards;
      animation-delay: var(--d);
    }
    .apparatus {
      opacity: 0;
      animation: mark-fade 0.7s var(--ease-out-quint) 0.5s forwards;
    }
  }
  @keyframes mark-draw {
    to {
      stroke-dashoffset: 0;
    }
  }
  @keyframes mark-fade {
    to {
      opacity: 1;
    }
  }
</style>
