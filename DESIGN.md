# Website design

## Purpose and pages
The static site follows the atkusage site's palette and terminal motif with different fonts: a quiet
paper background, orange accent and one terminal window. README.md remains the source of product
facts. index.html is deliberately short (about 300 words): a hero (one headline, one line, install command),
a demo video, a how-it-works diagram, agent steps with a snippet, four feature cards, focus matching and a one-line
local-by-design note. Each section is a heading plus one or two short lines; the demo, diagram and code carry the message. docs.html retains the complete reference with install, usage, exit-codes, ranking,
cache, languages and limits anchors. Wording about the network is exact: the binary makes no network
requests, while npx downloads the package.

## Color tokens

| Token | Light | Dark |
|---|---|---|
| Background | #f5f5f2 | #171816 |
| Surface | #ebebe6 | #262722 |
| Terminal | #fbfbf9 | #1d1e1a |
| Text | #2a2a28 | #f2f2eb |
| Muted | #61615a | #b7b7ab |
| Accent | #a02e08 | #ffab80 |
| Border | #b8b8ae | #68695f |

Dark values follow prefers-color-scheme. color-scheme adapts native controls. Text, muted text
and accent must meet WCAG AA (4.5:1 for ordinary text) on background, surface and terminal.
The filled brand and primary link reverse background and accent. Borders are decorative;
copy controls use the muted token so their boundaries remain visible.
Light accent #a02e08 measures 6.08:1 on surface, 6.65:1 on background and 7.02:1 on terminal;
dark accent #ffab80 measures 8.16:1 on surface.

## Type and spacing
Space Grotesk is the heading and prose face, self-hosted as fonts/space-grotesk-variable.woff2
(weights 300-700). Commit Mono is used for code, terminal chrome, labels and box-drawing
elements, from fonts/commit-mono-400.woff2 and fonts/commit-mono-700.woff2.
All three @font-face declarations use relative URLs, font-display: swap and system fallbacks.
Both fonts use SIL OFL 1.1; licenses are site/fonts/OFL-space-grotesk.txt and
site/fonts/OFL-commit-mono.txt. No reference-site font is used.

Body text is 1rem/1.65; labels and code are .875rem or larger; leads are 1.25rem. Landing h1
scales from 2.25rem to 4.5rem with -.035em tracking. The page caps at 62rem, prose at 44rem on the
landing page and 72ch in the reference. Side gutters are 1.5rem, then 1rem below 480px. Sections
are separated by 5rem (3.5rem below 760px). Smaller gaps use .25rem to 2.5rem steps.

## Components
- Header: plain wordmark and plain nav labels (Install, Docs, GitHub) in Commit Mono. The current
  page link carries aria-current and an accent underline; on the landing page no item is current.
- Hero: one column with the headline, one lead line and the install row. No wordmark above the h1.
- Install row: one surface-colored row with the command and a filled accent Copy button, vertically
  centered, command in a scrollable pre with no extra frame.
- Terminal: the only framed element (the demo video uses the frame). Rounded window and decorative dots.
- Demo video: a "See it work" section right after the hero, full content width, inside the same
  terminal frame (dots and bar). `media/demo.webm` with `demo.mp4` fallback, `media/poster.png`
  poster, native controls, muted loop. Light terminal theme matching the palette, recorded by
  `media/demo.tape` (vhs). A dark variant (`demo-dark.webm`, `demo-dark.mp4`, `poster-dark.png`, recorded by
  `media/demo-dark.tape`, identical except the theme) is chosen by the landing script via `matchMedia` before
  any video loads (`preload="none"`); without JavaScript the light files stay. README keeps the light GIF. Each
  file stays near or under 250 KB. The task comment must be visible in the final frame (map at most about 18 lines).
  Both use a one-line command in a committed copy of the repository without non-source files (see the tape header); check every frame for private data. A text fallback
  sits inside the video element.
- How-it-works diagram: an ordered list of seven surface-colored steps joined by short vertical
  connectors, numbered in accent mono, one-line plain descriptions; the ranking step has an accent
  left border to show where focus pulls. Step titles are 3 to 5 words with no prose
  under the list; from 960px the steps sit in three columns. Agent steps are a numbered list with accent numerals. Pure HTML and CSS, no images.
- Cards: borderless surface-colored blocks with a bold h3; two columns become one below 760px.
- Split sections: from 960px "Point it at your task" and "Use it with coding agents" are two
  columns (1fr 1fr, 3rem gap), text left and code block right; the block is `align-self: start` and
  sticky at top 24px so the short side never leaves a large blank. Snippet blocks wrap
  (`pre-wrap`, `overflow-wrap: anywhere`) and have no edge fade; only install commands and the map scroll.
- Text contrast: muted and caption text is at least 4.5:1 in both color schemes.
- Headings: plain h2 and h3, no numbering, rules or frames.
- Reference: h3 subheadings, short paragraphs and lists, tables scroll in focusable regions.  The flags table stacks each row (flag, then description) below 600px, CSS only.
  From 960px a 14rem sticky table of contents sits left of the content; below, it is a block
  above the content.
- Code: focusable, internally scrollable, never clipped or reflowed. Edge shadows (CSS only)
  show when more content is available to scroll; scrollbars have explicit colors and width.
- Footer: identical on both pages, same width as the content, thin top rule, small muted text in three short lines: version, Apache License 2.0 link and copyright; GitHub, npm, Changelog and Security links; a Credits link to docs.html#credits. Wraps cleanly at 360px. The existing SVG compass favicon stays.

## Copy script
Each page has one small inline script and no other JavaScript. The landing script also starts the demo video unless `prefers-reduced-motion: reduce` is set; with reduced motion, or without JavaScript, the video stays on its poster with native controls and only plays when asked. Copy buttons start hidden. They are
revealed only if the Clipboard API exists; otherwise users select the readable command.
Each data-copy attribute points to the code element whose textContent is copied.
Click disables the button while awaiting the write. Success shows Copied; rejection offers
Retry and tells users to select and copy manually. A polite live status announces the result.
No permission prompt is triggered on load, no HTML is injected, and no command is executed.
The command remains usable from disk even if clipboard access is unavailable.

## Rules and accessibility
Plain HTML and CSS, no build step, analytics, remote assets or external requests on page load.
External links are navigation only. Assets and local links remain relative for disk and
/symapper/ hosting. Keep one h1 per page, semantic landmarks, skip link, lang=en,
viewport metadata and visible focus. Do not use motion except the demo video, which never autoplays under reduced motion and always has controls. At 360px, content grids collapse
and code/table overflow stays within its own container; document scrollWidth must not exceed
the viewport at 360px and 390px on both pages.

## Preview
Open both HTML files from disk and under /symapper/. Inspect at 360px and desktop widths,
in light and dark mode. Follow local and contents links, navigate with a keyboard, scroll
long code, and check copying with clipboard access allowed, unavailable and rejected.
