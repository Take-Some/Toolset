# ddsinfo stability refactor

This is a cleaned-up dependency-free build of the legacy `ddsinfo` tool.

## What changed

- Removed the `RSN.Base` command-line parser dependency.
- Removed the `Tao.DevIl` managed assembly dependency.
- Removed the runtime dependency on native `DevIL.dll` for metadata inspection.
- Replaced image loading with a safe DDS header reader.
- Split the old `Program.cs` monolith into small modules.
- Replaced auto-incrementing assembly versioning with deterministic versions.
- Removed the old ILMerge release step.
- Added recoverable directory enumeration errors.
- Added non-zero exit codes for invalid paths/read failures.

## Files

- `Program.cs` - thin entry point.
- `DdsInfoApplication.cs` - application orchestration.
- `AppOptions.cs` - dependency-free CLI parser.
- `DdsHeaderReader.cs` - direct DDS/DX10 header parser.
- `DdsInfoRecord.cs` - metadata DTO.
- `FileScanner.cs` - safe file traversal.
- `ConsoleReporter.cs` - output formatting.

## CLI

```text
ddsinfo [options] <paths>

--version|-v
--help|-h
--recurse|-r
--dimensions|-d
--compression|-c
--format|-F
--mipmaps|-m
--strict|-s
--filter|-f <filter>
```

If no output field is selected, the tool defaults to dimensions and compression.

## Compatibility notes

The original `deps/DevIL.dll` and `scripts/DevIL.dll` files are left in the archive for historical compatibility, but the refactored C# app does not load them.
