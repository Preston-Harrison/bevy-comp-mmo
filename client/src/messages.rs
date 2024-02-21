use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{ROMFromServer, UMFromServer};

#[derive(Default, Resource)]
pub struct ServerMessageBuffer {
    pub unreliable: Vec<UMFromServer>,
    pub reliable_ordered: Vec<ROMFromServer>,
}

pub fn receive_messages(mut client: ResMut<RenetClient>, mut buffer: ResMut<ServerMessageBuffer>) {
    buffer.unreliable.clear();
    buffer.reliable_ordered.clear();

    while let Some(message) = client.receive_message(DefaultChannel::Unreliable) {
        if let Ok(um) = UMFromServer::try_from(message) {
            buffer.unreliable.push(um);
        } else {
            warn!("Received unparsable unreliable message from server");
        };
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        if let Ok(rom) = ROMFromServer::try_from(message) {
            buffer.reliable_ordered.push(rom);
        } else {
            warn!("Received unparsable reliable ordered message from server");
        }
    }
}
