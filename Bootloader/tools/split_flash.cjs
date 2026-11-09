// Выделяет реальные flash-сегменты из образа espflash; запись устройства не выполняется.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [input, output] = process.argv.slice(2);
const data = fs.readFileSync(input);
if (data.length <= 0x10000 || data[0] !== 0xe9 || data[0x10000] !== 0xe9 || data.readUInt16LE(0x8000) !== 0x50aa) {
  throw new Error('Unexpected ESP32-S3 flash layout');
}
const segments = [
  ['bootloader.bin', 0, 0x8000],
  ['partition-table.bin', 0x8000, 0x9000],
  ['firmware.bin', 0x10000, data.length],
].map(([file, offset, end]) => {
  const bytes = data.subarray(offset, end);
  fs.writeFileSync(path.join(output, file), bytes);
  return {file, offset, length: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex')};
});
// Symbolication is meaningful only with the exact ELF that produced these segments.
const elf = path.join(output, 'firmware.elf');
const debugSymbols = fs.existsSync(elf) ? {
  file: 'firmware.elf', sha256: crypto.createHash('sha256').update(fs.readFileSync(elf)).digest('hex'),
} : undefined;
fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify({chip: 'esp32s3', flashSize: 4194304, bootloaderProvider: 'espflash ESP-IDF second stage', segments, debugSymbols}, null, 2) + '\n');
