# Python Script Data Source Guide

## Overview

Aoba ships a RustPython-based embedded runner for Python data sources. Scripts execute inside a
dedicated interpreter thread and return structured station data to the Modbus master.

> Need the full CPython ecosystem or native extensions? Keep your script in a separate process and
> stream JSON to Aoba via `--data-source ipc:<path>`, reusing the existing IPC data source.

## RustPython Embedded Mode

- **CLI syntax**: `--data-source python:<path>` or `--data-source python:embedded:<path>`.
- The script is executed inside a dedicated RustPython interpreter thread.
- The runner returns a `PythonOutput` structure to the Modbus master. At present the
  implementation is still experimental: stdout capture is minimal, and scripts are expected to
  cooperate with helper functions (see `examples/cli_e2e/scripts/test_embedded.py`).
- Errors raised by the script are surfaced in the Aoba log stream.

### Example

```bash
cargo run -- --enable-virtual-ports \
  --master-provide-persist /tmp/vcom1 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --data-source python:embedded:$(pwd)/examples/cli_e2e/scripts/test_embedded.py
```

## Why Not CPython?

We removed the CPython subprocess runner because the IPC data source already streams JSON from
any external process, so maintaining a second embedded runtime only added complexity without new
capabilities.

## Migration Notes

- `python:external:<path>` now returns an error with guidance to use `ipc:<path>`.
- The CLI E2E suite no longer contains CPython-specific modules.
- Documentation and sample scripts were updated to focus on RustPython and IPC workflows.

## Limitations

- RustPython execution is still maturing; features such as stdout capture and richer integration
  helpers are planned but not yet implemented.
- Scripts that depend on native extensions or CPython-specific modules should continue to stream
  JSON through `--data-source ipc:<path>`.
