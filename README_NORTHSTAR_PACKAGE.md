# North Star stability package

This package contains two stabilized legacy tools:

- `ddsinfo/` - refactored dependency-free DDS metadata console application.
- `DevIL.NET/` - split/stabilized legacy C++/CLI DevIL.NET wrapper from the previous patch.

The `ddsinfo` app no longer depends on DevIL.NET, Tao.DevIl, RSN.Base, ILMerge, or native DevIL.dll for metadata inspection. The DevIL.NET wrapper is kept in this package as a separate stabilized interop component.
