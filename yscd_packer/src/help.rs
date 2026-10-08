pub fn print_help() {
    print!(
        "{}",
        r#"northstar-yscd-packer - Y Sound Cue Dictionary / NEF8

USAGE:
  northstar-yscd-packer pack   --input manifest.json --output audio.yscd [--source-root DIR] [--overwrite]
  northstar-yscd-packer unpack --input audio.yscd --output DIR [--entry CUE] [--overwrite]
  northstar-yscd-packer inspect --input audio.yscd
  northstar-yscd-packer verify  --input audio.yscd
  northstar-yscd-packer list    --input audio.yscd
  northstar-yscd-packer accepted-inputs
  northstar-yscd-packer doctor
  northstar-yscd-packer version

FORMAT:
  .yscd = NEF8(content_kind=34) -> binary YSCD v1 body
  body  = cue table + clip table + string table + embedded encoded audio payloads
  ref   = path/to/file.yscd@cue_name

PACK MANIFEST EXAMPLE:
{
  "schema": "newengine.yscd.manifest.v1",
  "version": 1,
  "cues": [
    {
      "name": "fire",
      "bus": "sfx",
      "spatial_policy": "spatial",
      "concurrency_group": "weapon.rifle.fire",
      "priority": 20,
      "gain_range": [0.98, 1.02],
      "pitch_range": [0.98, 1.02],
      "attenuation": {
        "min_distance": 1.0,
        "max_distance": 120.0,
        "curve": "inverse",
        "rolloff": 1.0
      },
      "clips": [
        { "name": "fire_a", "source": "audio/fire.wav", "weight": 1.0, "gain": 1.0, "pitch": 1.0 }
      ]
    }
  ]
}

SUPPORTED EMBEDDED AUDIO:
  .wav .ogg .opus .flac .mp3

PACK/UNPACK POLICY:
  Embedded audio is stored as encoded source bytes. The packer does not transcode it.
  Each clip carries BLAKE3 and is verified during inspect/verify/unpack.
"#
    );
}
