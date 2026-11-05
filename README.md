<p align="center">
  <img src="docs/logo.jpg" width="160" alt="Breaky">
</p>

# Breaky

A browser engine in Rust. Educational, experimental.

## Usage

```sh
cargo run -- page.html [style.css] [width] [out.png] [sel]
cargo run -- page.html base.css target.css width out.png [seconds]
cargo run -- fetch http://example.com [width] [out.png]
cargo run -- fetch https://example.com [width] [out.png]
cargo run -- anim <gif-or-image> [prefix]
cargo run -- animate <page> [style.css] [width] [prefix]
cargo run -- animate <url> [width] [prefix]
```

Prints the DOM tree, the styled tree with cascaded properties and the box
layout. With an output path it also paints the page to a PNG. The `sel`
flag renders with every text run selected, applying `::selection` styles.
With two stylesheets it renders the interpolated state between them at the
given time in seconds, honoring `transition` on either side. The `anim`
command dumps every frame of an image (local path or URL) as numbered PNGs
and prints each frame delay in milliseconds. The `animate` command renders a
whole page on a timer, stepping every animated GIF at its frame delay, for a
local file or a remote URL. The `click` command fires a click on an element
found by a CSS selector (at its rendered center) or at raw `X,Y` coordinates
hit-tested against the layout boxes, then paints the result. `mouse` fires
`mouseover` the same way, and with a second target it moves the pointer from
the first to the second, dispatching `mouseout` on the old element and
`mouseover` on the new one with `relatedTarget` set; `key` dispatches
`keydown` with `ev.key` set — at an explicit focus target
(`<selector|X,Y>` after the key) when given, else at
`document.activeElement`, else at `body` — bubbling to `document`.
`focus()`/`blur()` on elements dispatch non-bubbling `focus`/`blur` events
and maintain `document.activeElement`, which defaults to `body`. The `resize`
command sets the viewport (`document.innerWidth`/`innerHeight`) and dispatches
a `resize` event on `document` before painting. Scripts in
`<script>` tags (inline or `src`, local or remote) run in an embedded
QuickJS engine and print through `console.log`. Scripts get live DOM
bindings — `document.getElementById`, `document.body`,
`querySelector`/`querySelectorAll` (document and element scoped),
`createElement`/`createTextNode`/`cloneNode`/`appendChild`,
`insertBefore`/`removeChild`, `innerHTML`, `textContent`,
`getAttribute`/`setAttribute`, a `style` proxy for inline CSS
properties and `onclick` handlers — and mutations take effect before the page
is styled, laid out and painted. Element wrappers keep pointing at their nodes
across tree mutations, and `onclick` handlers survive moves and reindexing.
Text nodes are real wrappers with `data` and `nodeType`. Handlers get a real
event object — `type`, `target`, `currentTarget`, `clientX`/`clientY`, `key`,
`button`, `preventDefault`, `stopPropagation` — and
`addEventListener(type, fn, capture)` on elements and `document` dispatches
through capture and bubble phases up the ancestor chain, and pages can build
their own `new Event(type, { bubbles, cancelable })` with
`preventDefault`/`stopPropagation` and fire it with `dispatchEvent` on
elements and `document`. In event commands
`getBoundingClientRect()` returns the border box of the last layout
(`x`, `y`, `width`, `height`, `left`, `top`, `right`, `bottom`); during the
first script pass no layout exists yet, so it reports zeros.

## Architecture

Workspace of small crates:

- `breaky-html` — HTML tokenizer and DOM
- `breaky-css` — CSS parser
- `breaky-style` — selector matching and cascade
- `breaky-layout` — box tree and block/inline layout
- `breaky-render` — PNG painter (hand-written encoder, zero deps)
- `breaky-font` — TTF font engine with per-family registries
- `breaky-js` — embedded QuickJS interpreter (vendored C, FFI bindings) with mutable DOM bindings
- `breaky-img` — PNG, JPEG and GIF decoders with GIF animation frames (hand-written inflate, IDCT, LZW, zero deps)
- `breaky-net` — HTTP client and URL resolution
- `breaky-tls` — TLS 1.3 client with X.509 chain validation to the system trust store (hand-rolled, zero deps)
- `breaky` — CLI

## Roadmap

- [x] HTML tokenizer
- [x] DOM builder
- [x] CSS parser
- [x] Style engine
- [x] Inheritance
- [x] Typed values
- [x] Layout
- [x] Margin collapsing
- [x] Percentages
- [x] Floats
- [x] Rendering
- [x] Text and fonts
- [x] Networking
- [x] HTML entities
- [x] White-space processing
- [x] Text alignment
- [x] Vertical alignment
- [x] Lists
- [x] Tables
- [x] Table caption
- [x] Border collapse
- [x] List style image
- [x] Flexbox
- [x] Flex percentages
- [x] Flex wrap
- [x] Image decoding
- [x] Background images
- [x] Sticky positioning
- [x] Line breaks
- [x] JPEG decoder
- [x] Background position
- [x] Images (img element)
- [x] Image smoothing
- [x] Background size
- [x] GIF decoder
- [x] Object fit
- [x] Background color shorthand
- [x] Overflow
- [x] Z-index
- [x] Opacity
- [x] Min and max dimensions
- [x] Linear gradient
- [x] Border radius
- [x] Box shadow
- [x] Pseudo elements
- [x] Relative stacking contexts
- [x] Text shadow
- [x] Radial gradient
- [x] First letter
- [x] Selection
- [x] Transitions
- [x] Vector TTF font
- [x] HTTPS over std::net (TLS 1.3, AES-128/256-GCM, ChaCha20-Poly1305, SHA-256/SHA-384, X25519, RSA-PSS, ECDSA P-256)
- [x] Certificate chain validation to system roots (X.509, RSA PKCS#1/PSS, ECDSA P-256/P-384, PEM store)
- [x] Certificate chain cache for repeat fetches (images, stylesheets)
- [x] GIF animation (frames, delays, disposal methods)
- [x] Animated GIFs on remote pages (fetch, animate over timer)
- [x] Flex gap (shorthand cascade, wrap-aware)
- [x] @font-face with deferred web fonts, per family metrics
- [x] QuickJS scripting (`<script>` inline and src, console.log)
- [x] DOM bindings (getElementById, innerHTML, textContent, attributes, inline styles)
- [x] DOM construction (createElement, appendChild, insertBefore, removeChild, querySelector, stable wrappers across mutations)
- [x] Events (onclick handlers, `breaky click <selector|X,Y>` command)
- [x] addEventListener/removeEventListener with capture and bubbling, event object with click coordinates
- [x] Hit-testing: click coordinates resolved to DOM nodes through the layout boxes
- [x] querySelectorAll with comma-selector dedupe, element-scoped queries with document context
- [x] createTextNode/cloneNode, text node wrappers with mutable `data`
- [x] mouseover and keydown events (`breaky mouse`, `breaky key`), document-level listeners, `ev.key`
- [x] getBoundingClientRect() over the layout boxes
- [x] `new Event()` constructor and `dispatchEvent()` from JS, mouseover/mouseout pairs with previous-target tracking (`breaky mouse <from> <to>`)
- [x] focus()/blur() with non-bubbling focus/blur events, `document.activeElement`, keydown on the focused element
- [x] `resize` command with `document.innerWidth`/`innerHeight` and a resize event on `document`

## License

[MIT](LICENSE)
