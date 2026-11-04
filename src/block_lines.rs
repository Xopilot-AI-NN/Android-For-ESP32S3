//! Bounded line framing used only before binary USB block payloads.
#[derive(Default)]
pub struct Lines {
    len: usize,
}
impl Lines {
    pub fn with_prefix(len: usize) -> Self {
        Self { len }
    }
    pub fn push(&mut self, out: &mut [u8], byte: u8) -> Result<Option<usize>, ()> {
        match byte {
            b'\r' => Ok(None),
            b'\n' => {
                let len = self.len;
                self.len = 0;
                Ok(Some(len))
            }
            _ if self.len < out.len() => {
                out[self.len] = byte;
                self.len += 1;
                Ok(None)
            }
            _ => Err(()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interleaved_commands_preserve_the_following_binary_payload() {
        let data = b"@ZWIN|POWER\n@ZADB|CMD|6d656d6f7279\n@ZBLK|DATA|4|00000000\r\n\0\n\xff\r";
        let mut input = data.iter().copied();
        let mut buffer = [0; 256];
        let mut lines = Lines::default();
        let mut commands = 0;
        while let Some(byte) = input.next() {
            if let Some(n) = lines.push(&mut buffer, byte).unwrap() {
                if buffer[..n].starts_with(b"@ZBLK|") {
                    break;
                }
                commands += 1;
            }
        }
        assert_eq!(commands, 2);
        assert_eq!(input.collect::<alloc::vec::Vec<_>>(), b"\0\n\xff\r");
    }
    #[test]
    fn partial_control_prefix_and_overflow_are_bounded() {
        let mut buffer = [0; 16];
        buffer[..6].copy_from_slice(b"@ZWIN|");
        let mut lines = Lines::with_prefix(6);
        for b in b"POWER" {
            assert_eq!(lines.push(&mut buffer, *b), Ok(None));
        }
        assert_eq!(lines.push(&mut buffer, b'\n'), Ok(Some(11)));
        assert_eq!(&buffer[..11], b"@ZWIN|POWER");
        for _ in 0..16 {
            lines.push(&mut buffer, b'x').unwrap();
        }
        assert_eq!(lines.push(&mut buffer, b'x'), Err(()));
    }
}
