// Модуль реального состояния медиаплеера.
//! Playback comes from an encrypted companion, never from optimistic HID clicks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Playback {
    /// Общее состояние компонента.
    pub state: u8,
    /// Монотонное время/интервал position в миллисекундах.
    pub position_ms: u32,
    /// Монотонное время/интервал duration в миллисекундах.
    pub duration_ms: u32,
    /// Монотонное время/интервал received в миллисекундах.
    pub received_ms: u64,
}
impl Playback {
    /// Version 1 packet: version, state (0 stopped/1 paused/2 playing), two LE u32s.
    pub fn decode(packet: &[u8], now_ms: u64) -> Option<Self> {
        if packet.len() != 20 || packet[0] != 1 || packet[1] > 2 {
            return None;
        }
        let position_ms = u32::from_le_bytes(packet[2..6].try_into().ok()?);
        let duration_ms = u32::from_le_bytes(packet[6..10].try_into().ok()?);
        if position_ms > duration_ms || duration_ms > 604_800_000 {
            return None;
        }
        Some(Self {
            state: packet[1],
            position_ms,
            duration_ms,
            received_ms: now_ms,
        })
    }
    /// A companion heartbeat is required every 30 seconds to avoid stale state.
    pub fn progress(self, now_ms: u64) -> Option<(bool, u16)> {
        let elapsed = now_ms.saturating_sub(self.received_ms);
        if elapsed > 30_000 {
            return None;
        }
        let position = (u64::from(self.position_ms) + if self.state == 2 { elapsed } else { 0 })
            .min(u64::from(self.duration_ms));
        let progress = if self.duration_ms == 0 {
            0
        } else {
            (position * 1000 / u64::from(self.duration_ms)) as u16
        };
        Some((
            self.state == 2 && position < u64::from(self.duration_ms),
            progress,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет ход позиции при воспроизведении, остановку на паузе и истечение устаревших данных.
    fn playing_advances_paused_freezes_and_stale_metadata_expires() {
        let mut packet = [0; 20];
        packet[0] = 1;
        packet[1] = 2;
        packet[2..6].copy_from_slice(&1000u32.to_le_bytes());
        packet[6..10].copy_from_slice(&10000u32.to_le_bytes());
        let p = Playback::decode(&packet, 100).unwrap();
        assert_eq!(p.progress(1100), Some((true, 200)));
        assert_eq!(p.progress(40100), None);
        packet[1] = 1;
        let p = Playback::decode(&packet, 100).unwrap();
        assert_eq!(p.progress(1100), Some((false, 100)));
        packet[1] = 9;
        assert!(Playback::decode(&packet, 0).is_none());
    }
}
