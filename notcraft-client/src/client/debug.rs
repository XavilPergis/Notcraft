use std::time::Duration;

use notcraft_common::{
    debug::drain_debug_events,
    debug_events,
    world::{
        chunk::ChunkPos,
        chunk_aabb,
        debug::{WorldAccessEvent, WorldLoadEvent},
    },
};

use super::render::renderer::{add_debug_box, add_transient_debug_box, DebugBox, DebugBoxKind};

pub enum MesherEvent {
    Meshed { cheap: bool, pos: ChunkPos },
    MeshFailed(ChunkPos),
}

debug_events! {
    events,
    MesherEvent => "mesher",
}

// TODO: make the debug line renderer just a more generic line renderer and
// require it as a resource here.
pub fn debug_event_handler() {
    drain_debug_events::<WorldLoadEvent, _>(|event| match event {
        WorldLoadEvent::Loaded(pos) => add_transient_debug_box(Duration::from_secs(1), DebugBox {
            bounds: chunk_aabb(pos),
            rgba: [0.0, 1.0, 0.0, 0.8],
            kind: DebugBoxKind::Solid,
        }),
        WorldLoadEvent::Unloaded(pos) => {
            add_transient_debug_box(Duration::from_secs(1), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 0.0, 0.0, 0.8],
                kind: DebugBoxKind::Solid,
            })
        }
        WorldLoadEvent::Modified(pos) => {
            add_transient_debug_box(Duration::from_secs_f32(0.5), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 1.0, 0.0, 0.3],
                kind: DebugBoxKind::Dashed,
            })
        }
    });

    drain_debug_events::<WorldAccessEvent, _>(|event| match event {
        WorldAccessEvent::Read(pos) => add_debug_box(DebugBox {
            bounds: chunk_aabb(pos),
            rgba: [0.4, 0.4, 1.0, 0.1],
            kind: DebugBoxKind::Dotted,
        }),
        WorldAccessEvent::Written(pos) => add_debug_box(DebugBox {
            bounds: chunk_aabb(pos),
            rgba: [1.0, 0.8, 0.4, 0.1],
            kind: DebugBoxKind::Dotted,
        }),
        WorldAccessEvent::Orphaned(pos) => {
            add_transient_debug_box(Duration::from_secs(2), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 0.0, 0.0, 1.0],
                kind: DebugBoxKind::Solid,
            })
        }
    });

    drain_debug_events::<MesherEvent, _>(|event| match event {
        MesherEvent::Meshed { cheap: true, pos } => {
            add_transient_debug_box(Duration::from_secs(1), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 0.0, 1.0, 0.3],
                kind: DebugBoxKind::Dashed,
            })
        }
        MesherEvent::Meshed { cheap: false, pos } => {
            add_transient_debug_box(Duration::from_secs(1), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 1.0, 0.0, 0.3],
                kind: DebugBoxKind::Dashed,
            })
        }
        MesherEvent::MeshFailed(pos) => add_transient_debug_box(Duration::from_secs(2), DebugBox {
            bounds: chunk_aabb(pos),
            rgba: [1.0, 0.0, 0.0, 1.0],
            kind: DebugBoxKind::Solid,
        }),
    });
}
