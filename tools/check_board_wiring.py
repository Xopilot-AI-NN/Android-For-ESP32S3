#!/usr/bin/env python3
"""Source-only pinout regression check. Does not build or flash the board."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
boot = (ROOT / "src" / "boot.rs").read_text()
config = (ROOT / "src" / "config.rs").read_text()
display = (ROOT / "src" / "display.rs").read_text()

try:
    tft = boot.split('    #[cfg(all(feature = "display-st7789", not(feature = "display-software-spi")))]\n    let mut physical_display = {', 1)[1].split('    };', 1)[0]
    sd = boot.split('// SD requires >=74 clocks', 1)[1].split('let device = sd_spi::SdSpiSession::new', 1)[0]
except IndexError as e:
    raise SystemExit('FAIL: expected TFT/SD initialization block not found') from e

checks = {
    'TFT SPI3': 'p.SPI3' in tft,
    'TFT SCK GPIO1': '.with_sck(p.GPIO1)' in tft,
    'TFT MOSI GPIO2': '.with_mosi(p.GPIO2)' in tft,
    'TFT RST GPIO3': 'Output::new(p.GPIO3, Level::High' in tft,
    'TFT DC GPIO4': 'Output::new(p.GPIO4, Level::Low' in tft,
    'TFT CS GPIO5': 'Output::new(p.GPIO5, Level::High' in tft,
    'TFT BLK GPIO6 constant DC': (
        'crate::backlight::DcBacklight::new(Output::new(' in boot
        and 'p.GPIO6' in boot
        and 'let bl = dc_backlight;' in tft
        and 'backlight_ledc' not in boot
    ),
    'TFT panel BGR order': 'pub const TFT_MADCTL: u8 = 0x08;' in config,
    'Encoder hardware PCNT': 'BootKeys::new(p.PCNT, p.GPIO11, p.GPIO12, p.GPIO13)' in boot,
    'SD SPI2': 'p.SPI2' in sd,
    'SD MISO GPIO7': '.with_miso(p.GPIO7)' in sd,
    'SD SCK GPIO8': '.with_sck(p.GPIO8)' in sd,
    'SD MOSI GPIO9': '.with_mosi(p.GPIO9)' in sd,
    'SD CS GPIO10': 'Output::new(p.GPIO10, Level::High' in sd,
    'SD original 400 kHz, checked 100 kHz': 'if cfg!(feature = "sd-checked-startup")' in config and '400' in config,
    'SD 80 idle clocks': 'spi.write(&[0xFF; 10])' in sd,
    'Backlight after first clear': display.index('s.clear(BLACK)?;') < display.index('s.bl.set_high().map_err'),
}
failed = [name for name, passed in checks.items() if not passed]
for name, passed in checks.items():
    print(f"{'OK' if passed else 'FAIL'} {name}")
if failed:
    raise SystemExit(f'Wiring check failed: {len(failed)} issue(s)')
print('All board wiring checks passed. This does NOT validate runtime behavior on hardware.')
