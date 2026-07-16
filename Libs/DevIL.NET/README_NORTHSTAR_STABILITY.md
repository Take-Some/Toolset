# DevIL.NET stability refactor

This archive keeps the original Visual Studio 2005 / legacy Managed C++ project shape, but splits the previous `DevIL.NET.cpp` monolith into focused source files.

## File layout

- `DevIL.NET.h` — public DevIL.NET wrapper contract, enums, managed class declaration.
- `DevIL.NET.Internal.h` — internal helper declarations.
- `DevIL.NET.cpp` — static state, DevIL initialization, ILU loading, basic API methods.
- `DevIL.NET.Internal.cpp` — stride-aware bitmap/DevIL copy helpers and validation.
- `DevIL.NET.Load.cpp` — `LoadBitmap` / `LoadBitmapAndScale` implementation.
- `DevIL.NET.Save.cpp` — `SaveBitmap` / `NewBitMap` implementation.
- `DevIL.NET.Blit.cpp` — `Blit` implementation.
- `AssemblyInfo.cpp` — deterministic assembly version metadata.

## Stability fixes

- Every `LockBits` path has a matching `UnlockBits` on normal error paths.
- Bitmap row copy now respects `BitmapData::Stride`.
- Non-tight stride uses a tightly packed temporary buffer instead of assuming `width * 4` rows.
- `SaveBitmap` no longer mutates the caller-owned bitmap with in-place `RotateFlip`.
- `Blit` no longer uses a hard-coded `Bitmap(128, 128)` staging buffer.
- `Blit` no longer returns `NULL` from a `bool` method.
- ILU loading is attempted once and validates both required function pointers.
- Pixel buffer size math is checked before allocation/copy.
- Assembly version wildcard was replaced with deterministic version attributes.

## Notes

This refactor intentionally keeps old Managed C++ syntax (`__gc`, `__value`) so the project remains compatible with the original Visual Studio 2005-era toolchain.
