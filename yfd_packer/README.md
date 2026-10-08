# North Star YFD Packer

`.yfd` means North Star Font Dictionary.

It is a native `NEF8` ListFile font dictionary. The `d` suffix is intentional:
dictionary, not fragment. `YFT` remains free for a future fragment-like format.

Runtime selectors use the normal ListFile shape:

```text
fonts/ui.yfd@regular
fonts/ui.yfd@bold
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
