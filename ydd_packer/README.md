# NorthStar YDD Packer

In the NorthStar asset pipeline, .ydd is a resident NEF8 ListFile with:

    content_kind = drawable_dictionary

A single file may contain multiple models addressed through selectors:

    models/ped.ydd@body
    models/ped.ydd@head
    models/props.ydd@door_lod0

The tool does not introduce .ydd.json as an authoring format. Input is provided as real model source files:

    .obj
    .gltf
    .glb
    .fbx

FBX is recognized as an input kind; complete native FBX import remains a separate importer/provider concern.

## Commands

    northstar-ydd-packer pack -i body.obj -i head.glb -o assets/models/ped.ydd
    northstar-ydd-packer list -i assets/models/ped.ydd
    northstar-ydd-packer inspect -i assets/models/ped.ydd
    northstar-ydd-packer validate -i assets/models/ped.ydd

## Architecture rules

    NEF8 owns the outer ListFile container.
    YDD owns the resident drawable dictionary index and mesh payloads.
    AssetManager resolves bytes and format/container decoding.
    The model domain interprets drawable semantics.
    The renderer must not parse .ydd directly.

## Current importer coverage

OBJ is imported natively. glTF/GLB are imported through native accessor/buffer readers for POSITION, NORMAL, TEXCOORD_0, and indices.

FBX is recognized as a source kind but intentionally fails with a clear diagnostic until a dedicated FBX importer is available.
