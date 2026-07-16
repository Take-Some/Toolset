# Native Tooling Migration

> [!NOTE] REQUEST NOTE — current pass
> **У нас сейчас:** legacy tool identities are removed from the active workspace surface. Native tools are discovered through `tool.json` descriptors, cached under `.takesome/tools` during Script Env initialization, built through `tools build --safe`, and validated from descriptor-declared `validation_args`.
>
> **Было бы здорово:** next pass can deepen individual native capabilities, for example a full DDS cubemap converter or ListFile schema validator, without resurrecting old executable identities.
>
> **Technical details (EN):** active commands: `takesome.py tools scan/list/doctor/build/run`, `takesome.py tools build --safe --validate-after-build`, `takesome.py validate-build`, `devTools.bat`. Cache: `.takesome/tools/tool-registry.json`.

## Why this exists

The old tool names were separate products with their own assumptions:

```text
netexturetool
nepak
nematerialtool
nelistfile / nelisyfile
```

Those names are now treated as legacy identities. They must not return as launchers, Cargo packages, root commands, aliases, or build-script branches.

## Replacement model

```text
legacy reference -> native capability -> descriptor -> cache -> safe build surface -> command
```

The build path knows only the descriptor registry. It does not know hand-written tool names.

## Native capabilities introduced in this pass

| Capability | Native home | Replaces |
|---|---|---|
| `validation.build_surface` | `northstar.devspace` | manual build-script checks |
| `validation.legacy_tool_identity` | `northstar.devspace` + script-plane guard | old aliases returning silently |
| `validation.tool_registry` | `northstar.devspace validate-registry` + script-plane cache | manual tool lists / duplicate ids |
| `analysis.dds_header` | `northstar.devspace inspect-dds` | `DDSHeaderViewer.exe` |
| `analysis.cubemap_layout` | `northstar.devspace cubemap-layout` | `DDSCubemap.exe` layout knowledge |
| `analysis.asset_tree` | `northstar.devspace asset-scan` | `AssetAnalysisTool.exe` overview use case |
| `procedural.noise_smoke` | `northstar.devspace noise-smoke` | `NoiseGenerator.exe` smoke/test texture use case |
| `assets.chain.inspect` | `neassetchain` | ad-hoc asset relationship inspection |

## Invariant

```text
No active tool without descriptor.
No legacy identity as launcher.
No binary-only tool in build validation.
No manual tool list in script-plane logic.
Build scripts compile safe native tools from descriptors before plugin sync.
```
