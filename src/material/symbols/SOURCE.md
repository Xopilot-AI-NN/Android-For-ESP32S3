# Material Symbols Rounded

These are original Google Material Symbols Rounded, filled, weight 400,
optical size 24. They replace the watch's hand-drawn system icons.

Source: [google/material-design-icons](https://github.com/google/material-design-icons/tree/10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7/symbols/web)
at commit `10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7`.

Copyright Google LLC. Distributed under the [Apache 2.0 license](LICENSE).
The SVG paths are unchanged. The build tool rasterizes them with antialiasing
into 24 × 24 four-bit coverage masks; the firmware tints these with its theme.
All 26 masks together occupy 7,488 bytes of flash and require no font or SVG
parser on the ESP32. Circles, cards, sliders and clock numerals are separate
layout elements, not replacements for Material icon paths.

Rebuild locally with ImageMagick and Node:

```sh
node tools/build_ui_icons.cjs
```

To download the same upstream revision again, add `--fetch`. Normal firmware
builds use the checked-in atlas and do not access the network.
