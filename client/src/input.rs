use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{IdPlayerInput, InputBuffer, Player, PlayerInput, UMFromClient, UMFromServer};

use crate::{rollback::apply_game_sync, LocalPlayer};

pub fn read_inputs(
    mut commands: Commands,
    mut input_buffer: ResMut<InputBuffer>,
    local_player: Res<LocalPlayer>,
    keyboard_input: Res<Input<KeyCode>>,
    mut client: ResMut<RenetClient>,
    players_q: Query<(Entity, &Player)>,
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

    while let Some(message) = client.receive_message(DefaultChannel::Unreliable) {
        let Ok(unreliable_message) = UMFromServer::try_from(message) else {
            warn!("Failed to deserialize unreliable message");
            continue;
        };

        match unreliable_message {
            UMFromServer::IdPlayerInput(id_player_input) => {
                let IdPlayerInput(player_id, player_input) = id_player_input;
                input_buffer.0.insert(player_id, player_input);
            }
            UMFromServer::GameSync(game_sync) => {
                let players_q = players_q.iter().map(|(e, p)| (e, p)).collect::<Vec<_>>();
                apply_game_sync(&mut commands, game_sync, &players_q);
            }
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
    let mut players = players
        .iter_mut()
        .map(|(pos, transform)| (pos, transform.into_inner()))
        .collect::<Vec<_>>();

    common::process_input(&input_buffer, players.as_mut_slice(), time.delta_seconds());

    input_buffer.0.clear();
}
