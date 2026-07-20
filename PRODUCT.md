# Product

## Register

brand

## Users

Two audiences share one artifact. The reading's recipient revisits it as a keepsake — a personal document from a one-hour birth-chart reading, opened months later on a laptop or phone, possibly printed. The astrologer uses the same file as a working index — jumping to what was said about a planet, house, or aspect. Design leans keepsake; interactions lean tool.

## Product Purpose

Turn a recorded birth-chart reading into a single, self-contained, offline HTML artifact where the astrologer's *verbatim* words are filed under the chart elements they refer to, filterable by a computed chart wheel and element token bands. Success: the artifact feels like an heirloom document, works from `file://` forever, and every excerpt is traceable to its transcript span.

## Brand Personality

Engraved, celestial, exact. A plate from a 17th-century star atlas that happens to be interactive: etched line work, hairline ornament, plate borders — scientific reverence rather than new-age mysticism. Emotional goals: wonder, permanence, trust in provenance.

## Anti-references

- Generic "AI-generated dark dashboard": uniform pill chips, identical card grids, default border-radius everywhere, tracked-uppercase eyebrow labels on every section.
- New-age web mysticism: purple gradients, glowing neon zodiac, sparkle emoji energy.
- SaaS landing-page grammar of any kind. This is a document, not an app shell.

## Design Principles

- **The wheel is the hero plate.** Everything else is caption and apparatus around it, like a numbered figure in an atlas.
- **Verbatim is sacred.** The astrologer's words get the finest typography on the page; UI chrome stays recessive.
- **Engrave, don't decorate.** Ornament comes from line work, graduation ticks, and print conventions (rules, folios, small caps), never from glow, gradient, or blur.
- **One artifact, no dependencies.** Every design choice must survive `file://`, print, and a decade of neglect — self-contained fonts-by-stack, inline everything.
- **Tool speed under keepsake skin.** Filtering must stay one click/keystroke; ceremony never adds steps to the astrologer's workflow.

## Accessibility & Inclusion

Keyboard-accessible token bands mirror every wheel interaction (the wheel is enhancement, chips are the contract). Category color is never the only signal — glyphs, labels, and ring position carry identity (palette CVD-validated against the dark surface). Body text ≥4.5:1 on the indigo field. `prefers-reduced-motion` honored for any motion added.
