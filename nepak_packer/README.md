# North Star NEPAK Manager

First-party clean `.nepak` VFS package manager.

`.nepak` is a package/container layer for groups of runtime assets. It is intentionally separate from NEF8/ListFile assets such as `.ydd`, `.ytd`, `.ytyp`, `.nemat`, `.nepat`, and `.neui`.

## Commands

```bat
northstar-nepak-manager pack -i assets/runtime -o builds/runtime.nepak
northstar-nepak-manager inspect -i builds/runtime.nepak
northstar-nepak-manager manifest -i builds/runtime.nepak > manifest.json
northstar-nepak-manager list -i builds/runtime.nepak > entries.txt
northstar-nepak-manager verify -i builds/runtime.nepak
northstar-nepak-manager mount-test -i builds/runtime.nepak
northstar-nepak-manager diff --old builds/old.nepak --new builds/runtime.nepak
northstar-nepak-manager extract -i builds/runtime.nepak -o .takesome/extract/nepak --overwrite
```

## Clean format slice

```text
magic       = NEPAK\0\0\0
version     = 1.0
layout      = header + manifest + binary index + chunk table + aligned body
payload     = raw or deflate
integrity   = BLAKE3 for manifest, index, body, chunks and decoded entries
paths       = hardened normalized VFS package paths
semantics   = vfs_package_only
```

## Boundary rule

```text
.nepak = VFS package
NEF8/ListFile = semantic asset dictionaries
AssetManager = bytes/ranges/streams only
Domain gateways = meaning
Writer = explicit assets.container.nepak.writer capability
```
