# JSON Config Notes

Use the files in `assets/` as the starting point for user-facing configs.

## Rules

- Keep exactly one command block in the root object: `write_flash`, `read_flash`, `erase_flash`, `erase_region`, or `stub`.
- Keep hexadecimal addresses and sizes in `0x...` form.
- Let CLI flags override the JSON when the user asks for one-off changes at execution time.
- Keep `memory` as `nor` unless the user explicitly says `nand` or `sd`.

## Common Root Keys

The current CLI accepts these common keys:

- `chip`
- `memory`
- `port`
- `baud`
- `before`
- `after`
- `connect_attempts`
- `compat`
- `quiet`
- `stub_path`

## Minimal Write Example

```json
{
  "chip": "SF32LB52",
  "memory": "nor",
  "port": "/dev/ttyUSB0",
  "write_flash": {
    "verify": true,
    "erase_all": false,
    "no_compress": false,
    "files": [
      {
        "path": "app.bin",
        "address": "0x12020000"
      }
    ]
  }
}
```

## Minimal Read Example

```json
{
  "chip": "SF32LB52",
  "memory": "nor",
  "port": "/dev/ttyUSB0",
  "read_flash": {
    "files": [
      {
        "path": "firmware_backup.bin",
        "address": "0x12020000",
        "size": "0x00100000"
      }
    ]
  }
}
```

## Asset Map

- `assets/base-config.json`: editable starting template with common keys.
- `assets/write-flash-config.json`: ready-to-edit write example.
- `assets/read-flash-config.json`: ready-to-edit read example.
