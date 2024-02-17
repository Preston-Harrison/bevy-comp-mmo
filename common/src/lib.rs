use std::time::SystemTime;

use bevy::{prelude::*, utils::HashMap};
use bevy_renet::renet::Bytes;
use serde::{Deserialize, Serialize};

pub mod bundles;
pub mod rollback;
pub mod schedule;

pub const FRAME_DURATION_SECONDS: f64 = 1.0 / 60.0;
pub fn fixed_timestep_rate() -> Time<Fixed> {
    Time::<Fixed>::from_seconds(FRAME_DURATION_SECONDS)
}

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

#[macro_export]
macro_rules! impl_inner {
    ($outer:path, $inner:path) => {
        impl Into<$inner> for $outer {
            fn into(self) -> $inner {
                self.0
            }
        }

        impl Into<$inner> for &$outer {
            fn into(self) -> $inner {
                self.0
            }
        }

        impl Into<$outer> for $inner {
            fn into(self) -> $outer {
                $outer(self)
            }
        }
    };
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct PlayerId(pub u64);
impl_inner!(PlayerId, u64);

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
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

impl Player {
    pub fn new(id: PlayerId) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn with_speed(self, speed: f32) -> Self {
        Self { speed, ..self }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPlayerInput {
    pub x: i8,
    pub y: i8,
}

impl RawPlayerInput {
    pub fn at_frame(&self, frame: u64) -> FramedPlayerInput {
        FramedPlayerInput { raw: *self, frame }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FramedPlayerInput {
    pub raw: RawPlayerInput,
    pub frame: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IdPlayerInput {
    pub player_id: PlayerId,
    pub input: FramedPlayerInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Reliable Ordered Message from Server
pub enum ROMFromServer {
    PlayerConnected {
        player_id: PlayerId,
        server_object: ServerObject,
    },
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

#[derive(Clone, Default, Debug)]
pub struct InputBuffer(pub HashMap<PlayerId, RawPlayerInput>);

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
    /// Unix time this sync was generated in seconds.
    pub unix_time: f64,
    pub transforms: HashMap<ServerObject, Transform>,
    pub players: HashMap<ServerObject, Player>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Unreliable Message from Client
pub enum UMFromClient {
    PlayerInput(FramedPlayerInput),
}
impl_bytes!(UMFromClient);

#[derive(Default, Component, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct ServerObject(u64);

impl ServerObject {
    pub fn rand() -> Self {
        Self(rand::random())
    }
}

#[derive(Default, Resource)]
pub struct ServerEntityMap(pub HashMap<ServerObject, Entity>);

pub fn process_input<'a, I>(input_buffer: &InputBuffer, players: I, delta_time: f32)
where
    I: IntoIterator<Item = (&'a Player, &'a mut Transform)>,
{
    for (player, transform) in players {
        if let Some(input) = input_buffer.0.get(&player.id) {
            transform.translation.x += input.x as f32 * player.speed * delta_time;
            transform.translation.y += input.y as f32 * player.speed * delta_time;
        }
    }
}

pub fn get_unix_time() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

pub fn frames_since_unix_time(unix_time: f64) -> u64 {
    let current_time = get_unix_time();
    ((current_time - unix_time) / FRAME_DURATION_SECONDS) as u64
}

pub fn is_server() -> bool {
    std::env::var("SERVER").is_ok()
}
