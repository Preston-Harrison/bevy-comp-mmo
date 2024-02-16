use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{IdPlayerInput, InputBuffer, Player, PlayerInput, UMFromClient, UMFromServer};

use crate::{messages::ServerMessageBuffer, LocalPlayer};

pub fn read_inputs(
    mut input_buffer: ResMut<InputBuffer>,
    local_player: Res<LocalPlayer>,
    keyboard_input: Res<Input<KeyCode>>,
    server_messages: Res<ServerMessageBuffer>,
) {
    // Collect local player input.
    let mut input = PlayerInput::default();
    if keyboard_input.pressed(KeyCode::W) {
        input.y += 1;
    }
    if keyboard_input.pressed(KeyCode::S) {
        input.y -= 1;
    }
    if keyboard_input.pressed(KeyCode::A) {
        input.x -= 1;
    }
    if keyboard_input.pressed(KeyCode::D) {
        input.x += 1;
    }

    if input != PlayerInput::default() {
        input_buffer.0.insert(local_player.id, input);
    }

    for message in server_messages.unreliable.iter() {
        match message {
            UMFromServer::IdPlayerInput(id_player_input) => {
                let IdPlayerInput(player_id, player_input) = id_player_input;
                input_buffer.0.insert(*player_id, *player_input);
            }
            _ => {}
        }
    }
}

pub fn broadcast_local_input(
    input_buffer: ResMut<InputBuffer>,
    local_player: Res<LocalPlayer>,
    mut client: ResMut<RenetClient>,
) {
    let local_input = input_buffer.0.get(&local_player.id);
    if let Some(input) = local_input {
        info!("Broadcasting input {:?}", input);
        client.send_message(
            DefaultChannel::Unreliable,
            UMFromClient::PlayerInput(*input),
        );
    }
}

pub fn process_inputs(
    mut input_buffer: ResMut<InputBuffer>,
    mut players: Query<(&Player, &mut Transform)>,
    time: Res<Time>,
) {
    let players = players
        .iter_mut()
        .map(|(pos, transform)| (pos, transform.into_inner()));

    common::process_input(&input_buffer, players, time.delta_seconds());

    input_buffer.0.clear();
}
