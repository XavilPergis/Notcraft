//! this module provides tracking for which chunks need to be meshed.
//!
//! this tracking is needed because we can only mesh a chunk properly when all
//! 26 neighboring chunks are loaded, so directly meshing a chunk when a ["chunk
//! added" event][crate::common::world::ChunkEvent] is received is off the
//! table. tracking is handled by [`MeshTracker`], which receives updates about
//! the state of the world, and produces positions of chunks that have enough
//! data to be meshed.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use nalgebra::Point3;

use notcraft_common::{
    prelude::*,
    transform::Transform,
    world::{
        chunk::{ChunkSection, ChunkSectionPos},
        VoxelWorld, WorldEvent,
    },
};

fn neighbors<F>(pos: ChunkSectionPos, mut func: F)
where
    F: FnMut(ChunkSectionPos),
{
    for x in pos.x - 1..=pos.x + 1 {
        for y in pos.y - 1..=pos.y + 1 {
            for z in pos.z - 1..=pos.z + 1 {
                let neighbor = ChunkSectionPos { x, y, z };
                if neighbor != pos {
                    func(neighbor);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct MeshTracker {
    constraining: HashMap<ChunkSectionPos, HashSet<ChunkSectionPos>>,
    constrained_by: HashMap<ChunkSectionPos, HashSet<ChunkSectionPos>>,

    needs_mesh: HashSet<ChunkSectionPos>,

    loaded: HashSet<ChunkSectionPos>,
    terrain_entities: HashMap<ChunkSectionPos, Entity>,
}

impl MeshTracker {
    // INVARIANT: if `have_data` does not contain X, then `constrained_by` also does
    // not contain X

    // INVARIANT: if `have_data` contains X, then `constraining` does NOT contain X

    // INVARIANT: for a chunk X and each value Y of `constraining[X]`,
    // `constrained_by[Y]` must contain X

    fn constrain_self(&mut self, center: ChunkSectionPos) {
        neighbors(center, |neighbor| {
            if !self.loaded.contains(&neighbor) {
                self.constraining
                    .entry(neighbor)
                    .or_default()
                    .insert(center);
                self.constrained_by
                    .entry(center)
                    .or_default()
                    .insert(neighbor);
            }
        });
    }

    fn constrain_neighbors(&mut self, center: ChunkSectionPos) {
        neighbors(center, |neighbor| {
            if self.loaded.contains(&neighbor) {
                self.constraining
                    .entry(center)
                    .or_default()
                    .insert(neighbor);
                self.constrained_by
                    .entry(neighbor)
                    .or_default()
                    .insert(center);

                self.needs_mesh.remove(&neighbor);
            }
        });
    }

    fn unconstrain_self(&mut self, center: ChunkSectionPos) {
        let constrainers = match self.constrained_by.remove(&center) {
            Some(constrainers) => constrainers,
            None => return,
        };

        for &constrainer in constrainers.iter() {
            let neighbor_constraining = self
                .constraining
                .get_mut(&constrainer)
                .expect("(remove) constraints not bidirectional");

            neighbor_constraining.remove(&center);
            if neighbor_constraining.is_empty() {
                self.constraining.remove(&constrainer);
            }
        }
    }

    fn unconstrain_neighbors(&mut self, center: ChunkSectionPos) {
        let constraining = match self.constraining.remove(&center) {
            Some(constraining) => constraining,
            None => return,
        };

        for &neighbor in constraining.iter() {
            let neighbor_constraints = self
                .constrained_by
                .get_mut(&neighbor)
                .expect("(add) constraints not bidirectional");

            neighbor_constraints.remove(&center);
            if neighbor_constraints.is_empty() {
                self.constrained_by.remove(&neighbor);
                self.needs_mesh.insert(neighbor);
            }
        }
    }

    pub fn chunk_mesh_failed(&mut self, chunk: ChunkSectionPos) {
        // by the time it gets here, the failed chunk might have been unloaded itself,
        // or might have had its neighbors been unloaded. if it was unloaded itself,
        // there is nothing to do because of the `have_data` invariants.
        if !self.loaded.contains(&chunk) {
            return;
        }

        self.constrain_self(chunk);

        // it might be the case that a mesh failed because of unloaded neighbors, but
        // between the time that the failed response was queued and now, the neighbors
        // became loaded.
        self.request_mesh(chunk);
    }

    pub fn add_chunk(&mut self, chunk: ChunkSectionPos, cmd: &mut Commands) {
        let success = self.loaded.insert(chunk);
        assert!(
            success,
            "chunk {:?} was added to tracker, but it was already tracked",
            chunk
        );

        let world_pos: Point3<f32> = chunk.origin().origin().into();
        let transform = Transform::from(world_pos);
        self.terrain_entities
            .insert(chunk, cmd.spawn().insert(transform).id());

        // set up constraints for the newly-added chunk
        self.constrain_self(chunk);

        // remove constraints for neighbors that depended on us
        self.unconstrain_neighbors(chunk);

        // it may be the case that we get a new chunk where all its neighbors already
        // have data, in which case the new chunk is already unconstrained.
        self.request_mesh(chunk);

        assert!(!self.constraining.contains_key(&chunk));
    }

    pub fn remove_chunk(&mut self, chunk: ChunkSectionPos, cmd: &mut Commands) {
        let success = self.loaded.remove(&chunk);
        assert!(
            success,
            "chunk {:?} was removed from tracker, but it wasn't tracked",
            chunk
        );

        let entity = self.terrain_entities.remove(&chunk).unwrap();
        cmd.entity(entity).despawn();

        // remove old `constraining` entries that pointed to the removed chunk,
        // upholding one of our `have_data` invariants.
        self.unconstrain_self(chunk);

        // add constraints to neighbors of the newly-removed chunk
        self.constrain_neighbors(chunk);

        assert!(!self.constrained_by.contains_key(&chunk));
    }

    pub fn request_mesh(&mut self, chunk: ChunkSectionPos) {
        let is_unconstrained = !self.constrained_by.contains_key(&chunk);
        let is_loaded = self.loaded.contains(&chunk);
        if is_unconstrained && is_loaded {
            self.needs_mesh.insert(chunk);
        }
    }

    pub fn next(&mut self, world: &Arc<VoxelWorld>) -> Option<Arc<ChunkSection>> {
        let &pos = self.needs_mesh.iter().next()?;
        let chunk = world.section(pos);
        assert!(
            chunk.is_some(),
            "chunk {:?} was tracked for meshing but didnt exist in the world",
            pos
        );
        assert!(
            !self.constrained_by.contains_key(&pos),
            "chunk {:?} was in to-mesh set, but was constrained by {:?}",
            pos,
            self.constrained_by[&pos]
        );
        self.needs_mesh.remove(&pos);
        chunk
    }

    pub fn terrain_entity(&self, pos: ChunkSectionPos) -> Option<Entity> {
        self.terrain_entities.get(&pos).cloned()
    }
}

pub fn update_tracker(
    mut cmd: Commands,
    mut tracker: ResMut<MeshTracker>,
    mut events: EventReader<WorldEvent>,
) {
    for event in events.iter() {
        match event {
            WorldEvent::LoadedSection(chunk) => tracker.add_chunk(chunk.pos(), &mut cmd),
            WorldEvent::UnloadedSection(chunk) => tracker.remove_chunk(chunk.pos(), &mut cmd),
            WorldEvent::ModifiedSection(chunk) => {
                // NOTE: we're choosing to keep chunk meshes for chunks that have already been
                // meshed, but no longer have enough data to re-mesh
                tracker.request_mesh(chunk.pos());
            }

            _ => {}
        }
    }
}
