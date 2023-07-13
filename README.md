# Notcraft

Notcraft is a sandboxy voxel game that I've been using to test out game programming stuff. I'm really not quite sure what I want to do with it?

**This project is super barebones! And very buggy!**

## Controls

Some controls listed here are subject to change in the future, as they are placeholders for yet unimplemented systems, including block picking and mouse grabbing.

### Movement
- `W`: Move forwards
- `S`: Move backwards
- `A`: Move left
- `D`: Move right
- `Space`: Jump
### Miscellaneous
- `Ctrl+C`: Toggle mouse grab
- `Q`: Switch block used for placement
- `Ctrl+shift+S`: Fix the camera in its current transform (easy to do by accident when using area select, can fix via the input below)
- `Ctrl+shift+F`: Make the camera follow the player entity
### Terrain Manipulation
- `E`: Destroy sphere of blocks
- `Ctrl(Hold)`: Increase movement speed
- `LeftClick`: Destroy one block
- `RightClick`: Place one block
- `Ctrl+RightClick`: Place line of blocks to player
- `Ctrl+Shift+LeftClick`: Destroy area of blocks
- `Ctrl+Shift+RightClick`: Place area of blocks

## Command Line Arguments

- `--mesher-mode <simple|greedy>`: Changes whether the chunk mesher uses greedy meshing (combining similar block faces) or simple meshing (only does block face occlusion culling). currently, greedy meshing looks a bit strange due to randomized face textures, and is significantly slower
- `-D, --enable-debug-events <events>...`: Debug events with names listed here will be toggled on, and may be processed by a debug event sink, like the one in `notcraft-client/src/client/debug.rs`. If the flag is specified, but no event names are given, then all debug events are enabled. The currently supported debug events are:
  - `world-load`: Chunk loading/unloading/modification events
  - `world-access`: Chunk reading/writing/orphaning events
  - `mesher`: Chunk meshing events

# Hacking

## Static
- Player movement and terrain manipulation code can be found in `notcraft-client/src/main.rs` (gotta clean that up lol)

- The dynamic shader loader, along with the texture loader can be found in `notcraft-client/src/client/loader.rs`

- The main renderer can be found in `notcraft-client/src/client/render/renderer.rs`
  
- The chunk mesher, responsible for creating static terrain meshes out of chunk data can be found split across all the files in `notcraft-client/src/client/render/mesher`
  - `mod.rs` is the main driver, exposing a `ChunkMesherPlugin` that listens for chunk events and spins off meshing tasks
  - `tracker.rs` exposes `MeshTracker`, which keeps track of which chunks have enough loaded neighbor chunks to create chunk meshes for themselves.
  - `generation.rs` is the actual vertex buffer generation code

- Terrain collision code can be found in `notcraft-common/src/physics.rs`, along with basic acceleration/velocity application code

- Chunk generation code (i.e. the stuff responsible for actually sshaping the world) can be found in `notcraft-common/src/world/generation.rs`

- Chunk management code and the main world struct can be found in `notcraft-common/src/world/mod.rs`, and chunk internals can be found in `notcraft-common/src/world/chunk.rs`

## Dynamic

Dynamically-loaded resources are placed in `resources`, including audio files, textures, and shaders. Additionally, gameplay resources can be found there too, like `resources/blocks.json`, which describes a list of blocks and their properties to be loaded into the game upon startup.

## Shaders

Notcraft includes a shader hot-reloading feature by default, as well as a crude preprocessor that allows for `#pragma include`-ing of other shader files. Saving a shader file while the game is running will cause itself and all dependants (via `#pragma include`) of itself to be recompiled and swapped in.

- `resources/shaders/terrain` contains the shaders used to draw the voxel terrain
  
- `resources/shaders/post.glsl` contains post-processing code, and is where fog is applied
  
- `resources/shaders/sky.glsl` contains skybox drawing code, creating the sky gradient and the cloud layer
  
- `resources/shaders/adjustables.glsl` contains a bunch of `#define`s for various constants and whatnot used int other shaders