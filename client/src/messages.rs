use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{ROMFromServer, UMFromServer, FRAME_DURATION_SECONDS};

use crate::ARGS;

#[derive(Default, Resource, Clone)]
pub struct ServerMessages {
    pub unreliable: Vec<UMFromServer>,
    pub reliable_ordered: Vec<ROMFromServer>,
}

/// Keeps a history of server messages.
/// [most recent, ..., oldest]
/// Length of buffer will be determined by the read latency.
#[derive(Default, Resource)]
pub struct ServerMessageBuffer(VecDeque<ServerMessages>);

impl ServerMessageBuffer {
    fn has_buffer(&self) -> bool {
        self.0.len() as f32 * FRAME_DURATION_SECONDS as f32
            > ARGS.get().unwrap().network_latency / 1000.0
    }

    fn read(&mut self) -> ServerMessages {
        if self.has_buffer() {
            self.0.pop_back().unwrap()
        } else {
            ServerMessages::default()
        }
    }

    fn write(&mut self, buffer: ServerMessages) {
        self.0.push_front(buffer);
    }
}

pub fn receive_messages(
    mut client: ResMut<RenetClient>,
    mut buffer: ResMut<ServerMessageBuffer>,
    mut messages: ResMut<ServerMessages>,
) {
    let mut next_buffer = ServerMessages::default();

    while let Some(message) = client.receive_message(DefaultChannel::Unreliable) {
        if let Ok(um) = UMFromServer::try_from(message) {
            next_buffer.unreliable.push(um);
        } else {
            warn!("Received unparsable unreliable message from server");
        };
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        if let Ok(rom) = ROMFromServer::try_from(message) {
            next_buffer.reliable_ordered.push(rom);
        } else {
            warn!("Received unparsable reliable ordered message from server");
        }
    }

    buffer.write(next_buffer);
    *messages = buffer.read();
}
