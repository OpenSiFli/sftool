# Troubleshooting

## `sftool` Is Missing

- Check `SFTOOL_BIN` first. If it is set, use that binary path.
- Otherwise check whether `sftool` resolves from `PATH`.
- If neither exists, stop and return:
  - `Error: sftool command not found in PATH or SFTOOL_BIN. Stop and ask the user to install sftool or fix the environment before continuing.`

## Serial Port Fails to Open

- Confirm the exact port name from the user: `COMx` on Windows, `/dev/tty*` on Linux, `/dev/cu.*` or `/dev/tty.*` on macOS.
- Check whether another tool already owns the port.
- Check permissions on Unix-like systems before changing command flags.

## Timeouts or Verify Failures

- Reconfirm the chip type, memory type, baud rate, and serial port.
- Retry with `--compat` before proposing more invasive changes.
- Keep `--verify` enabled unless the user explicitly wants a faster but less safe path.
- Avoid suggesting `--erase-all` as a generic fix.

## JSON Config Fails

- Confirm there is exactly one command block in the file.
- Confirm every address and size string starts with `0x`.
- Confirm file lists are non-empty for `write_flash`, `read_flash`, and `stub` subcommands.
- Use `sftool config <file>` only after the JSON is internally consistent.

## Chip or Memory Mismatch

- Supported chips in the current CLI are `SF32LB52`, `SF32LB55`, `SF32LB56`, and `SF32LB58`.
- Supported memory types are `nor`, `nand`, and `sd`.
- Keep `memory` at `nor` unless the user says otherwise.
