# Command Recipes

Validate the current CLI with `sftool --help` and the matching subcommand `--help` before using any recipe.

## Flash Firmware

Prefer `write_flash` for normal programming. Default to `--verify` unless the user explicitly disables verification.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash --verify app.bin@0x12020000
```

Use multiple files when the image set already has fixed target addresses.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash --verify \
  bootloader.bin@0x12010000 \
  app.bin@0x12020000
```

Use `--compat` only for conservative transfers after repeated timeouts or verify failures.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 --compat write_flash --verify app.bin@0x12020000
```

Do not add `--erase-all` unless the user explicitly wants a full erase before programming.

## Read Back Flash

Use `read_flash` for backups or binary dumps. The argument format is `<path@address:size>`.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 read_flash firmware_backup.bin@0x12020000:0x00100000
```

Use Windows serial ports exactly as provided by the user.

```bash
sftool -c SF32LB52 -p COM7 read_flash firmware_backup.bin@0x12020000:0x00100000
```

## Erase a Region

Use `erase_region` only for targeted erases. The argument format is `<address:size>`.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_region 0x12020000:0x00100000
```

Do not suggest `erase_flash` unless the user explicitly asks to wipe the entire flash.

## Execute a JSON Config

Use `config` when the user wants a reusable JSON file for the same operation.

```bash
sftool config sftool_param.json
```

CLI flags before `config` can override the JSON file.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 config sftool_param.json
```

## Help Probes

Use these probes whenever the environment might differ from the examples:

```bash
sftool --help
sftool write_flash --help
sftool read_flash --help
sftool erase_region --help
sftool config --help
```
