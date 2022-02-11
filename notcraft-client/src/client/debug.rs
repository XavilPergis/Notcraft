use std::time::Duration;

use notcraft_common::{
    aabb::Aabb,
    debug::drain_debug_events,
    debug_events,
    world::{
        chunk::{ChunkSectionPos, CHUNK_LENGTH},
        chunk_section_aabb,
        debug::{WorldAccessEvent, WorldLoadEvent},
        ChunkPos,
    },
};

use super::render::renderer::{add_debug_box, add_transient_debug_box, DebugBox, DebugBoxKind};

pub enum MesherEvent {
    Meshed { cheap: bool, pos: ChunkSectionPos },
    MeshFailed(ChunkSectionPos),
}

debug_events! {
    events,
    MesherEvent => "mesher",
}

pub fn debug_chunk_aabb(pos: ChunkPos) -> Aabb {
    let len = CHUNK_LENGTH as f32;
    let min = len * nalgebra::point![pos.x as f32, 24.0, pos.z as f32];
    let max = min + len * nalgebra::vector![1.0, 0.0, 1.0];
    Aabb { min, max }
}

// TODO: make the debug line renderer just a more generic line renderer and
// require it as a resource here.
pub fn debug_event_handler() {
    drain_debug_events::<WorldLoadEvent, _>(|event| match event {
        WorldLoadEvent::Loaded(pos) => add_transient_debug_box(
            Duration::from_secs(1),
            DebugBox::new(debug_chunk_aabb(pos))
                .with_color([0.0, 1.0, 0.0, 0.8])
                .with_kind(DebugBoxKind::Solid),
        ),
        WorldLoadEvent::Unloaded(pos) => add_transient_debug_box(
            Duration::from_secs(1),
            DebugBox::new(debug_chunk_aabb(pos))
                .with_color([1.0, 0.0, 0.0, 0.8])
                .with_kind(DebugBoxKind::Solid),
        ),
        WorldLoadEvent::Modified(pos) => add_transient_debug_box(
            Duration::from_secs_f32(0.5),
            DebugBox::new(debug_chunk_aabb(pos))
                .with_color([1.0, 1.0, 0.0, 0.3])
                .with_kind(DebugBoxKind::Dashed),
        ),
        WorldLoadEvent::LoadedSection(pos) => add_transient_debug_box(
            Duration::from_secs(1),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([0.0, 1.0, 0.0, 0.8])
                .with_kind(DebugBoxKind::Solid),
        ),
        WorldLoadEvent::UnloadedSection(pos) => add_transient_debug_box(
            Duration::from_secs(1),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 0.0, 0.0, 0.8])
                .with_kind(DebugBoxKind::Solid),
        ),
        WorldLoadEvent::ModifiedSection(pos) => add_transient_debug_box(
            Duration::from_secs_f32(0.5),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 1.0, 0.0, 0.3])
                .with_kind(DebugBoxKind::Dashed),
        ),
    });

    drain_debug_events::<WorldAccessEvent, _>(|event| match event {
        WorldAccessEvent::Read(pos) => add_debug_box(
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([0.4, 0.4, 1.0, 0.1])
                .with_kind(DebugBoxKind::Dotted),
        ),
        WorldAccessEvent::Written(pos) => add_debug_box(
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 0.8, 0.4, 0.1])
                .with_kind(DebugBoxKind::Dotted),
        ),
        WorldAccessEvent::Orphaned(pos) => add_transient_debug_box(
            Duration::from_secs(2),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 0.0, 0.0, 1.0])
                .with_kind(DebugBoxKind::Solid),
        ),
    });

    drain_debug_events::<MesherEvent, _>(|event| match event {
        MesherEvent::Meshed { cheap: true, pos } => add_transient_debug_box(
            Duration::from_secs(1),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 0.0, 1.0, 0.3])
                .with_kind(DebugBoxKind::Dashed),
        ),
        MesherEvent::Meshed { cheap: false, pos } => add_transient_debug_box(
            Duration::from_secs(1),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 1.0, 0.0, 0.3])
                .with_kind(DebugBoxKind::Dashed),
        ),
        MesherEvent::MeshFailed(pos) => add_transient_debug_box(
            Duration::from_secs(2),
            DebugBox::new(chunk_section_aabb(pos))
                .with_color([1.0, 0.0, 0.0, 1.0])
                .with_kind(DebugBoxKind::Solid),
        ),
    });
}
