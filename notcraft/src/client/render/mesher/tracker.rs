use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::common::world::{
    chunk::{Chunk, ChunkPos},
    VoxelWorld,
};

fn neighbors<F>(pos: ChunkPos, mut func: F)
where
    F: FnMut(ChunkPos),
{
    for x in pos.x - 1..=pos.x + 1 {
        for y in pos.y - 1..=pos.y + 1 {
            for z in pos.z - 1..=pos.z + 1 {
                let neighbor = ChunkPos { x, y, z };
                if neighbor != pos {
                    func(neighbor);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct MeshTracker {
    constraining: HashMap<ChunkPos, HashSet<ChunkPos>>,
    constrained_by: HashMap<ChunkPos, HashSet<ChunkPos>>,
    unconstrained: HashSet<ChunkPos>,

    have_data: HashSet<ChunkPos>,
}

impl MeshTracker {
    // INVARIANT: if `have_data` does not contain X, then `constrained_by` also does
    // not contain X

    // INVARIANT: if `have_data` contains X, then `constraining` does NOT contain X

    // INVARIANT: for a chunk X and each value Y of `constraining[X]`,
    // `constrained_by[Y]` must contain X

    pub fn chunk_mesh_failed(&mut self, chunk: ChunkPos) {
        // by the time it gets here, the failed chunk might have been unloaded itself,
        // or might have had its neighbors been unloaded. if it was unloaded itself,
        // there is nothing to do because of the `have_data` invariants.
        if !self.have_data.contains(&chunk) {
            return;
        }

        neighbors(chunk, |neighbor| {
            if !self.have_data.contains(&neighbor) {
                self.constraining.entry(neighbor).or_default().insert(chunk);
                self.constrained_by
                    .entry(chunk)
                    .or_default()
                    .insert(neighbor);
            }
        });

        // it might be the case that a mesh failed because of unloaded neighbors, but
        // between the time that the failed response was queued and now, the neighbors
        // became loaded.
        if !self.constrained_by.contains_key(&chunk) {
            self.unconstrained.insert(chunk);
        }
    }

    pub fn chunk_added(&mut self, chunk: ChunkPos) {
        self.have_data.insert(chunk);

        // set up constraints for the newly-added chunk
        neighbors(chunk, |neighbor| {
            if !self.have_data.contains(&neighbor) {
                self.constraining.entry(neighbor).or_default().insert(chunk);
                self.constrained_by
                    .entry(chunk)
                    .or_default()
                    .insert(neighbor);
            }
        });

        // it may be the case that we get a new chunk where all its neighbors already
        // have data, in which case the new chunk is already unconstrained.
        if !self.constrained_by.contains_key(&chunk) {
            self.unconstrained.insert(chunk);
        }

        // remove constraints for neighbors that depended on us
        if let Some(constraining) = self.constraining.get_mut(&chunk) {
            for &neighbor in constraining.iter() {
                let neighbor_constraints = self
                    .constrained_by
                    .get_mut(&neighbor)
                    .expect("(add) constraints not bidirectional");

                neighbor_constraints.remove(&chunk);
                if neighbor_constraints.is_empty() {
                    self.unconstrained.insert(neighbor);
                    self.constrained_by.remove(&neighbor);
                }
            }

            self.constraining.remove(&chunk);
        }

        assert!(!self.constraining.contains_key(&chunk));
    }

    pub fn chunk_removed(&mut self, chunk: ChunkPos) {
        self.have_data.remove(&chunk);

        // add constraints to neighbors of the newly-removed chunk
        neighbors(chunk, |neighbor| {
            if self.have_data.contains(&neighbor) {
                self.constraining.entry(chunk).or_default().insert(neighbor);
                self.constrained_by
                    .entry(neighbor)
                    .or_default()
                    .insert(chunk);

                self.unconstrained.remove(&neighbor);
            }
        });

        // remove old `constraining` entries that pointed to the removed chunk,
        // upholding one of our `have_data` invariants.
        if let Some(constrainers) = self.constrained_by.get(&chunk) {
            for &constrainer in constrainers.iter() {
                let neighbor_constraining = self
                    .constraining
                    .get_mut(&constrainer)
                    .expect("(remove) constraints not bidirectional");

                neighbor_constraining.remove(&chunk);
                if neighbor_constraining.is_empty() {
                    self.constraining.remove(&constrainer);
                }
            }
        }

        // uphold our second `have_data` invariant.
        self.constrained_by.remove(&chunk);
    }

    pub fn chunk_modified(&mut self, chunk: ChunkPos) {
        if self.have_data.contains(&chunk) {
            self.unconstrained.insert(chunk);
        }
    }

    pub fn next(&mut self, world: &Arc<VoxelWorld>) -> Option<Arc<Chunk>> {
        let &pos = self.unconstrained.iter().next()?;
        self.unconstrained.remove(&pos);
        let chunk = world.chunk(pos);
        assert!(
            chunk.is_some(),
            "chunk {:?} was tracked for meshing but didnt exist in the world",
            pos
        );
        chunk
    }
}
