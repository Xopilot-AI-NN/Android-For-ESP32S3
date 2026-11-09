# Software RTC

`src/rtc.rs` is the sole owner of AOSP Wear wall-clock state.

Sources, in priority/behavior order:

1. Wi-Fi SNTP when **Automatic time** is enabled.
2. Paired-phone companion time when supplied.
3. Manual `Date & time` adjustment when automatic time is disabled.
4. Persisted local-time holdover after reboot.
5. Unsynchronised monotonic clock starting at 00:00 on a clean boot.

USB/desktop-viewer traffic never sets wall time.

The board has no battery-backed calendar RTC, so the holdover cannot know how long the board was fully unpowered. This is corrected at the next Wi-Fi/companion sync.
