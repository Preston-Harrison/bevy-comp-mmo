use bevy::{prelude::*, utils::HashMap};
use bevy_renet::renet::Bytes;
use serde::{Deserialize, Serialize};

pub mod bundles;

macro_rules! impl_bytes {
    ($t:ty) => {
        impl Into<Bytes> for $t {
            fn into(self) -> Bytes {
                let encoded = bincode::serialize(&self).unwrap();
                Bytes::copy_from_slice(&encoded)
            }
        }

        impl TryFrom<Bytes> for $t {
            type Error = bincode::Error;

            fn try_from(bytes: Bytes) -> Result<Self, bincode::Error> {
                bincode::deserialize(&bytes)
            }
        }
    };
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct PlayerId(pub u64);

#[derive(Component)]
pub struct Player {
    pub id: PlayerId,
    pub speed: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            id: PlayerId(0),
            speed: 100.0,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub x: i8,
    pub y: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdPlayerInput(pub PlayerId, pub PlayerInput);

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Reliable Ordered Message from Server
pub enum ROMFromServer {
    PlayerConnected(PlayerId),
    PlayerDisconnected(PlayerId),
    GameSync(GameSync),
}
impl_bytes!(ROMFromServer);

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Reliable Ordered Message from Client
pub enum ROMFromClient {
    PlayerLogin(PlayerLogin),
}
impl_bytes!(ROMFromClient);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerLogin {
    pub id: PlayerId,
}

#[derive(Resource, Default)]
pub struct InputBuffer(pub HashMap<PlayerId, PlayerInput>);

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Unreliable Message from Server
pub enum UMFromServer {
    IdPlayerInput(IdPlayerInput),
    GameSync(GameSync),
}
impl_bytes!(UMFromServer);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSync {
    pub frame: u64,
    pub players: HashMap<PlayerId, Transform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Unreliable Message from Client
pub enum UMFromClient {
    PlayerInput(PlayerInput),
}
impl_bytes!(UMFromClient);

#[derive(Resource, Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct FrameCount(pub u64);

pub fn process_input(
    input_buffer: &InputBuffer,
    players: &mut [(&Player, &mut Transform)],
    delta_time: f32,
) {
    for (player, transform) in players.iter_mut() {
        if let Some(input) = input_buffer.0.get(&player.id) {
            transform.translation.x += input.x as f32 * player.speed * delta_time;
            transform.translation.y += input.y as f32 * player.speed * delta_time;
        }
    }
}
