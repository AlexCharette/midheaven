// What the app shows a person about a file on disk.
//
// A path is backend detail; a basename is what someone recognizes. Written
// twice, in the two components that each render three "browse…" buttons.

/** The last segment of a path, whichever separator the platform uses. */
export const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;
