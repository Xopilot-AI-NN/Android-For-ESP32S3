// Rebuild the checked-in 4-bit coverage atlas; requires Node and ImageMagick.
// Roboto attribution and license: src/material/fonts/SOURCE.md and LICENSE.
const fs = require('node:fs');
const cp = require('node:child_process');
const path = require('node:path');
const output = path.resolve(__dirname, '../src/material/fonts');
fs.mkdirSync(output, {recursive: true});
const uiChars = [
    ...Array.from({length: 95}, (_, i) => String.fromCharCode(i + 32)),
    ...Array.from({length: 64}, (_, i) => String.fromCharCode(i + 0x410)),
    'Ё', 'ё',
];
for (const [name, size, cellW, cellH, chars, font] of [
    ['body', 11, 22, 24, uiChars, 'Roboto-Regular.ttf'],
    ['label', 16, 22, 24, uiChars, 'Roboto-Regular.ttf'],
    ['clock', 64, 48, 84, Array.from('0123456789:'), 'Roboto-Light.ttf'],
]) {
    const data = [], widths = [];
    for (const ch of chars) {
        const code = ch.charCodeAt(0);
        if (code === 32) {
            widths.push(Math.ceil(size / 3));
            data.push(...Array(cellW * cellH / 2).fill(0));
            continue;
        }
        // %% escapes ImageMagick's property interpolation.
        const label = ch === '%' ? '%%' : ch === '\\' ? '\\\\' : ch;
        const args = ['-background', 'black', '-fill', 'white', '-font',
            process.env.WATCH_UI_FONT || path.join(output, font),
            '-pointsize', String(size), `label:${label}`];
        const width = Number(cp.execFileSync('magick', [...args, '-format', '%w', 'info:']).toString());
        widths.push(code === 32 ? Math.ceil(size / 3) : width);
        const pixels = cp.execFileSync('magick', [...args, '-gravity', 'northwest', '-extent', `${cellW}x${cellH}`, '-depth', '8', 'gray:-']);
        if (pixels.length !== cellW * cellH || width > cellW) throw new Error(`Invalid glyph ${code}`);
        for (let i = 0; i < pixels.length; i += 2) {
            data.push((Math.round(pixels[i] / 17) << 4) | Math.round(pixels[i + 1] / 17));
        }
    }
    fs.writeFileSync(path.join(output, `${name}.bin`), Buffer.from(data));
    fs.writeFileSync(path.join(output, `${name}-widths.bin`), Buffer.from(widths));
}
