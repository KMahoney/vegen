# VeGen VS Code Extension

Syntax highlighting and language server integration for `vegen` (.vg templates).

## Configuration

The following settings are exposed under `vegen`:

| Setting            | Description                                                    |
| ------------------ | -------------------------------------------------------------- |
| `vegen.serverPath` | Absolute path to the `vegen` executable.                       |
| `vegen.serverArgs` | Extra arguments appended after `lsp` when spawning the server. |

## Troubleshooting

- Set `vegen.serverPath` to the `vegen` executable. See https://github.com/KMahoney/vegen for instructions.
- Check the _VeGen Language Server_ output channel for server logs and transport errors.
