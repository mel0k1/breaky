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
```

Prints the DOM tree, the styled tree with cascaded properties and the box
layout. With an output path it also paints the page to a PNG. The `sel`
flag renders with every text run selected, applying `::selection` styles.
With two stylesheets it renders the interpolated state between them at the
given time in seconds, honoring `transition` on either side.

## Architecture

Workspace of small crates:

- `breaky-html` — HTML tokenizer and DOM
- `breaky-css` — CSS parser
- `breaky-style` — selector matching and cascade
- `breaky-layout` — box tree and block/inline layout
- `breaky-render` — PNG painter (hand-written encoder, zero deps)
- `breaky-font` — embedded bitmap font and glyph metrics
- `breaky-img` — PNG, JPEG and GIF decoders (hand-written inflate, IDCT, LZW, zero deps)
- `breaky-net` — HTTP client and URL resolution
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

## License

[MIT](LICENSE)
