// Pointer-drag for the instrument rings: grab anywhere on a ring's hit
// annulus and turn it about the plate's centre. Emits *angular deltas*
// (degrees, clockwise positive, unwrapped across ±180°) converted by the
// ring's gear ratio into minutes — the caller accumulates them on one
// unbounded instant, so carrying past midnight or New Year needs no cases.

export type RingDragOptions = {
  /** The rings SVG, measured once per gesture for the rotation centre. */
  svg: () => SVGSVGElement;
  /** Gear ratio: clockwise degrees → minutes of civil time. */
  degToMinutes: (deg: number) => number;
  onstart: () => void;
  onmove: (minutes: number) => void;
  onend: () => void;
};

export function ringDrag(node: SVGElement, opts: RingDragOptions) {
  let dragging = false;
  let last = 0;
  let cx = 0;
  let cy = 0;

  // Screen angle at the pointer, degrees, clockwise positive (y grows down).
  const angle = (e: PointerEvent) =>
    (Math.atan2(e.clientY - cy, e.clientX - cx) * 180) / Math.PI;

  function down(e: PointerEvent) {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    // the browser must not start a text selection or native drag from the
    // ring; preventDefault also suppresses focus, so restore that by hand
    // (keyboard users still reach the slider by clicking or tabbing)
    e.preventDefault();
    (node as SVGElement & { focus: () => void }).focus?.();
    const r = opts.svg().getBoundingClientRect();
    cx = r.left + r.width / 2;
    cy = r.top + r.height / 2;
    dragging = true;
    last = angle(e);
    node.setPointerCapture(e.pointerId);
    opts.onstart();
  }

  function move(e: PointerEvent) {
    if (!dragging) return;
    const a = angle(e);
    const d = ((a - last) % 360 + 540) % 360 - 180; // unwrap across ±180°
    last = a;
    opts.onmove(opts.degToMinutes(d));
  }

  function up(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    node.releasePointerCapture(e.pointerId);
    opts.onend();
  }

  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  return {
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", up);
    },
  };
}
