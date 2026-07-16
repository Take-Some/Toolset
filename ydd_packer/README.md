# North Star YDD Packer

`.ydd` в North Star Engine — это resident `NEF8 ListFile` с `content_kind = drawable_dictionary`.
Он хранит набор моделей внутри одного файла и адресует их как:

```text
models/ped.ydd@body
models/ped.ydd@head
models/props.ydd@door_lod0
```

Инструмент не вводит `.ydd.json` как authoring format. Вход — реальные model sources:

```text
.obj
.gltf
.glb
.fbx   recognized; full native FBX importer is a separate provider pass
```

## Commands

```bat
northstar-ydd-packer pack -i body.obj -i head.glb -o assets/models/ped.ydd
northstar-ydd-packer list -i assets/models/ped.ydd
northstar-ydd-packer inspect -i assets/models/ped.ydd
northstar-ydd-packer validate -i assets/models/ped.ydd
```

## Architecture rules

```text
NEF8 owns the outer ListFile container.
YDD body owns the resident drawable dictionary index and mesh payloads.
AssetManager resolves bytes only.
engine.model interprets drawable semantics.
Renderer must not parse .ydd directly.
```

## Current importer slice

OBJ is imported natively. glTF/GLB are imported through native accessor/buffer readers for POSITION, NORMAL, TEXCOORD_0 and indices. FBX is recognized as a source kind but intentionally fails with a clear diagnostic until a dedicated FBX importer provider is added.
