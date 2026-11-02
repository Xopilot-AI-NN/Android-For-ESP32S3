# Material Symbols Rounded

These are original Google Material Symbols Rounded, filled, weight 400,
optical size 24. They replace the watch's hand-drawn system icons.

Source: [google/material-design-icons](https://github.com/google/material-design-icons/tree/10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7/symbols/web)
at commit `10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7`.

Copyright Google LLC. Distributed under the [Apache 2.0 license](LICENSE).
The SVG paths are unchanged. The build tool rasterizes them with antialiasing
into 24 × 24 four-bit coverage masks and separate 30 × 30 launcher masks;
the firmware tints these with its theme. The 31 system masks occupy 8,928 bytes;
the larger launcher atlas occupies 13,950 bytes. No font or SVG parser is
needed on the ESP32. Circles, cards, sliders and clock numerals are separate
layout elements, not replacements for Material icon paths.

Rebuild locally with ImageMagick and Node:

```sh
node tools/build_ui_icons.cjs
```

To download the same upstream revision again, add `--fetch`. Normal firmware
builds use the checked-in atlas and do not access the network.
