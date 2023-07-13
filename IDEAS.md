# async world

**PROS**:
- allows for straightforward, nonblocking world manipulation code
```rs
async fn modify_world(world: &Arc<VoxelWorld>, pos: BlockPos, block: BlockId) {
    let mut cache = ChunkCache::new(world);
    // might have to load or generate the chunk first, so async allows us to not have to handle that complexity every time we want to access the world.
    for dy in 0..10 {
        cache.set(pos.offset(Direction::Up, dy), block).await;
    }
}

async fn modify_world2(world: &Arc<VoxelWorld>, pos: BlockPos, block: BlockId) {
    let mut patch = ChunkPatch::new(world);
    for dy in 0..10 {
      patch.set(pos.up(dy), block);
    }

    // might have to load or generate the chunk first, so async allows us to not have to handle that complexity every time we want to access the world.
    patch.apply().await;
}
```

- *mutable world, mutable chunks*:
  - simplest approach
  - either one thread handles world access, or a lock must be held the entire time a chunk is being read/written.
- *fully concurrent world*:
  - very complicated to implement world
  - likely not much better than just locking the world each time
  
- *move chunks behind an `Arc<RwLock>`*:
  - still quite simple
  - r/w world access does not lock world for very long
  - concurrent w access + r/w access in the same chunk will block one of the threads until modification is complete
- *move chunks behind an `Orphan`*:
  - still quite simple
  - r/w world access does not lock world for very long
  - concurrent r access is completely nonblocking, but may get slightly old world data
  - concurrent w access will only block other w accesses
- *anything + sync chunk cache*:
  - nonblocking reads/writes for cache hits
  - nonblocking writes are always possible: chunk writes can be remembered, and applied later, when the target chunk is loaded, or perhaps with an explicit flush
  - missed chunk reads may have to block the calling thread entirely, until the chunks can be generated/loaded and cached.
- *async world access*:
  - very simple, straighforward world modification code
  - completely nonblocking
- *read-only access, write queue, sync writes at end of each tick*:
  - nonblocking reads are always possible
  - nonblocking writes are always possible
  - simple to implement
  - world is not updated immediately, may mor may not be a big issue