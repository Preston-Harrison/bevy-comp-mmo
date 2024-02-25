use bevy_renet::renet::ClientId;
use common::PlayerId;

pub struct Lobby {
    players: Vec<(ClientId, PlayerId)>,
}
