# Roboto UI font

Roboto is Android's system UI font. This copy is the official Google Fonts
Roboto Regular and Light fonts from [googlefonts/roboto](https://github.com/googlefonts/roboto/tree/38062f4b4a0be4346d07a928408da21602545e9e/src/hinted),
commit `38062f4b4a0be4346d07a928408da21602545e9e`.
See the included [Apache 2.0 license](LICENSE) for terms and attribution.

`Roboto-Regular.ttf` and `Roboto-Light.ttf` are the local source assets. `tools/build_ui_font.cjs`
rasterizes printable ASCII plus the Russian Cyrillic alphabet (including Ё/ё)
at 14, 18 and 36 px, and clock digits at 64 px, into four-bit antialiased coverage
atlases, with proportional glyph advances. Firmware builds include those
atlases directly; the watch needs no font service, network or TTF parser.
This replaces the previous Noto Sans UI atlases. The large watch-clock numerals
use Roboto Light at 64 px, replacing the hand-drawn digit paths. Larger UI
labels also use Roboto rather than the compact boot/debug font. Only the
separate boot/debug display retains its small bitmap font.

To rebuild from the checked-in font:

```sh
node tools/build_ui_font.cjs
```

The UI atlas now includes 166 glyphs: ASCII, Russian Cyrillic, degree, middle
dot, ellipsis, em dash and minus. The 36 px atlas is rasterized directly from
Roboto Regular rather than magnifying an 18 px bitmap.
