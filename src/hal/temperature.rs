//! ESP32-S3 die temperature, sampled independently of drawing and cached for diagnostics.
//! Register sequence and calibration follow ESP-IDF v5.4's S3 temperature_sensor_ll.h.
use core::sync::atomic::{AtomicI32, Ordering};
const UNAVAILABLE: i32 = i32::MIN;
static LAST: AtomicI32 = AtomicI32::new(UNAVAILABLE);

/// Tenths of a degree Celsius. This is the silicon temperature, not room temperature.
pub fn snapshot() -> Option<i32> {
    match LAST.load(Ordering::Relaxed) {
        UNAVAILABLE => None,
        value => Some(value),
    }
}

fn calibrated(raw: u8, offset: i32, calibration: u16) -> i32 {
    // eFuse uses sign/magnitude in tenths of a degree, not two's complement.
    let delta = if calibration & 0x100 != 0 {
        -i32::from(calibration & 0xff)
    } else {
        i32::from(calibration & 0xff)
    };
    let scaled = i32::from(raw) * 4386 - offset * 278800 - 205200;
    (scaled + if scaled < 0 { -500 } else { 500 }) / 1000 - delta
}

#[cfg(target_arch = "xtensa")]
pub fn sample() {
    use esp_hal::{
        delay::Delay,
        efuse::{Efuse, TEMP_CALIB},
        peripherals::SENS,
        time::Instant,
    };
    static NEXT_MS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
    let now = Instant::now().duration_since_epoch().as_millis() as u32;
    let next = NEXT_MS.load(Ordering::Relaxed);
    if next != 0 && (now.wrapping_sub(next) as i32) < 0 {
        return;
    }
    NEXT_MS.store(now.wrapping_add(1000), Ordering::Relaxed);
    let sens = SENS::regs();
    let read = |dac: u8| -> Option<u8> {
        critical_section::with(|_| unsafe {
            // Only change the TSENS DAC nibble: ADC/Wi-Fi use other analog fields.
            unsafe extern "C" {
                fn esp_rom_regi2c_read(block: u8, host: u8, reg: u8) -> u8;
                fn rom_i2c_writeReg(block: u8, host: u8, reg: u8, value: u8);
            }
            sens.sar_peri_clk_gate_conf()
                .modify(|_, w| w.tsens_clk_en().set_bit());
            let previous = esp_rom_regi2c_read(0x69, 1, 6);
            rom_i2c_writeReg(0x69, 1, 6, (previous & 0xf0) | dac);
            sens.sar_tsens_ctrl2()
                .modify(|_, w| w.sar_tsens_xpd_force().bits(1));
            sens.sar_tsens_ctrl().modify(|_, w| {
                w.sar_tsens_clk_div()
                    .bits(6)
                    .sar_tsens_power_up_force()
                    .set_bit()
                    .sar_tsens_power_up()
                    .set_bit()
            });
        });
        Delay::new().delay_micros(250);
        sens.sar_tsens_ctrl()
            .modify(|_, w| w.sar_tsens_dump_out().set_bit());
        let start = Instant::now();
        let result = loop {
            let value = sens.sar_tsens_ctrl().read();
            if value.sar_tsens_ready().bit_is_set() {
                break Some(value.sar_tsens_out().bits());
            }
            // A broken sensor must never freeze the UI or the block-device bridge.
            if start.elapsed().as_micros() >= 2000 {
                break None;
            }
        };
        sens.sar_tsens_ctrl().modify(|_, w| {
            w.sar_tsens_dump_out()
                .clear_bit()
                .sar_tsens_power_up()
                .clear_bit()
                .sar_tsens_power_up_force()
                .clear_bit()
        });
        sens.sar_tsens_ctrl2()
            .modify(|_, w| unsafe { w.sar_tsens_xpd_force().bits(0) });
        result
    };
    let calibration = if Efuse::rtc_calib_version() == 1 {
        Efuse::read_field_le::<u16>(TEMP_CALIB)
    } else {
        0
    };
    let value = read(15).and_then(|raw| {
        let value = calibrated(raw, 0, calibration);
        if value > 750 {
            read(5)
                .map(|raw| calibrated(raw, -2, calibration))
                .filter(|v| (500..=1250).contains(v))
        } else if (-100..=800).contains(&value) {
            Some(value)
        } else {
            None
        }
    });
    LAST.store(value.unwrap_or(UNAVAILABLE), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversion_applies_signed_factory_calibration() {
        assert_eq!(calibrated(0, 0, 0), -205);
        assert_eq!(calibrated(150, 0, 0), 453);
        assert_eq!(calibrated(150, 0, 12), 441);
        assert_eq!(calibrated(150, 0, 0x100 | 12), 465);
        assert_eq!(calibrated(150, -2, 0), 1010);
    }
}
