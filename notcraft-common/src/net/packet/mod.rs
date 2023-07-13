#[derive(Clone, Debug)]
pub enum ClientToServerLoginPacket {
    Login { username: String },
}

#[derive(Clone, Debug)]
pub enum ServerToClientLoginPacket {
    LoginAck {
        initial_position: Point3<f32>,
        initial_pitch: f32,
        initial_yaw: f32,
    },
}

#[derive(Clone, Debug)]
pub enum ClientToServerPlayPacket {
    UpdateTransform {
        position: Point3<f32>,
        pitch: f32,
        yaw: f32,
    },
    BreakBlock {
        position: BlockPos,
    },
    PlaceBlock {
        id: BlockId,
        position: BlockPos,
    },
}

#[derive(Clone, Debug)]
pub enum ServerToClientPlayPacket {
    ChunkData { data: Box<[BlockId]> },
}
