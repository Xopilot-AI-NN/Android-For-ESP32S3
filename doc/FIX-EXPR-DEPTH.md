# v0.2.1 / Bootloader 1.5.1 parser fix

The v0.2.0 Material userspace could fail during Rhai compilation with:

`ParseError(ExprTooDeep, 151:87)`

Cause: Bootloader 1.5.0 configured Rhai with a function-expression depth of 16,
while the Material input-policy bundle contained a nested state update at line 151.

Fixes:
- Bootloader 1.5.1 uses `set_max_expr_depths(64, 32)` while keeping the operation
  and call-level limits in place.
- Firmware 0.2.1 flattens theme/DND/airplane state transitions into temporary
  variables so normal UI logic does not sit on the parser limit.
