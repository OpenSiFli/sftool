---
name: sftool
description: Use sftool for SiFli flashing, flash readback, JSON config generation, and serial troubleshooting on SF32LB52, SF32LB55, SF32LB56, and SF32LB58 devices.
---

# SFTool

Use this skill to drive the local `sftool` CLI for normal SiFli workflows.

## Runtime Guardrails

- Resolve the executable before planning any real command. Prefer `SFTOOL_BIN` when it is set; otherwise use `sftool` from `PATH`.
- Use a shell-appropriate equivalent of these checks:

```sh
if [ -n "${SFTOOL_BIN:-}" ]; then
  "$SFTOOL_BIN" --version
elif command -v sftool >/dev/null 2>&1; then
  sftool --version
else
  exit 1
fi
```

```powershell
if ($env:SFTOOL_BIN) {
  & $env:SFTOOL_BIN --version
} elseif (Get-Command sftool -ErrorAction SilentlyContinue) {
  sftool --version
} else {
  exit 1
}
```

- If the executable check fails, stop and return exactly:
  - `Error: sftool command not found in PATH or SFTOOL_BIN. Stop and ask the user to install sftool or fix the environment before continuing.`
- Do not try to auto-install `sftool`, invent a binary path, or continue with hypothetical commands after that error.
- Do not guess `--chip`, `--port`, addresses, sizes, or file paths.
- Prefer `write_flash --verify` unless the user explicitly disables verification.
- Do not use `erase_flash` or `write_flash --erase-all` unless the user explicitly asks for an entire-flash erase.
- Re-run `sftool --help` or the relevant subcommand help in the current environment before constructing the final command whenever there is any doubt about the arguments or options.

## Workflow

1. Resolve `SFTOOL_BIN` or `sftool`.
2. Run `sftool --help` and the relevant subcommand help.
3. Gather the missing required arguments:
   - Flashing: chip, port, file list, target addresses, and non-default memory type.
   - Readback: chip, port, output path, address, and size.
   - JSON config: command type plus the same required fields as the equivalent CLI command.
4. Choose the safest matching operation:
   - `write_flash` for programming.
   - `read_flash` for dumps and backups.
   - `erase_region` for targeted erases.
   - `config` for repeatable JSON-driven runs.
5. Use conservative flags first:
   - Keep the default memory as `nor` unless the user says `nand` or `sd`.
   - Use `--compat` only when the user reports repeated timeouts or verify failures, or asks for the safest transfer mode.
6. If the user asks about `stub` workflows, inspect `sftool stub --help` in the current environment before advising.

## Reference Map

- Use [`references/commands.md`](./references/commands.md) for normal command construction and examples.
- Use [`references/config-examples.md`](./references/config-examples.md) when the user wants JSON config files or automation-friendly templates.
- Use [`references/troubleshooting.md`](./references/troubleshooting.md) for missing commands, serial errors, timeout handling, and verify failures.
- Copy from [`assets/base-config.json`](./assets/base-config.json), [`assets/write-flash-config.json`](./assets/write-flash-config.json), and [`assets/read-flash-config.json`](./assets/read-flash-config.json) instead of recreating JSON templates from scratch.
