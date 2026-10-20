// Whether the reading is being presented — the astrologer sharing the chart
// with a client, over a call or a second screen.
//
// App-level rather than the reading view's own, because presenting clears
// chrome the view does not own: the layout's preferences gear has no place on
// a screen someone else is looking at.

const state = $state({ on: false });

export const isPresenting = (): boolean => state.on;

export function setPresenting(on: boolean) {
  state.on = on;
}
