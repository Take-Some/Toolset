# North Star NEFTD Packer

`.neftd` means North Star Font Dictionary.

It is a native `NEF8` ListFile font dictionary. The `d` suffix is intentional:
dictionary, not fragment. `YFT` remains free for a future fragment-like format.

Runtime selectors use the normal ListFile shape:

```text
fonts/ui.neftd@regular
fonts/ui.neftd@bold
```

Supported source formats:

```text
ttf
otf
woff
woff2
ttc
```

Commands: `create`, `pack`, `inspect`, `list`, `validate`, `extract`.
