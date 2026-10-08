# North Star YSCD Packer

`northstar-yscd-packer` builds and extracts **Y Sound Cue Dictionary** (`.yscd`) assets.

A `.yscd` is a self-contained NEF8 ListFile (`content_kind=34`). Its inflated body starts with `YSCD` and contains:

- addressable Cue entries (`file.yscd@cue`);
- Cue playback metadata (bus, looping, concurrency, priority, spatial policy, attenuation, gain/pitch randomization);
- weighted clip variants;
- optional layered Cue playback (`near`, `far`, `tail`, `body`, `aux`) with per-layer gain/pitch and attenuation overrides;
- the encoded audio bytes themselves;
- per-clip BLAKE3 hashes.

The tool intentionally does **not** transcode audio. Packing and unpacking preserve the embedded `.wav/.ogg/.opus/.flac/.mp3` bytes exactly.

## Commands

```text
northstar-yscd-packer pack --input manifest.json --output rifle.yscd
northstar-yscd-packer inspect --input rifle.yscd
northstar-yscd-packer verify --input rifle.yscd
northstar-yscd-packer list --input rifle.yscd
northstar-yscd-packer unpack --input rifle.yscd --output unpacked --overwrite
northstar-yscd-packer unpack --input rifle.yscd --entry fire --output fire_only --overwrite
```

## Intended runtime reference

```text
shared/audio/weapon/rifle/rifle.yscd@fire
shared/audio/weapon/rifle/rifle.yscd@reload
```

Runtime integration is owned by `engine.audio`; `engine.assets` owns only VFS and the generic NEF8 envelope.
