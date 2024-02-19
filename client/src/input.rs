use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{
    rollback::{InputRollback, RollbackRequest, SyncFrameCount},
    IdPlayerInput, Player, RawPlayerInput, UMFromClient, UMFromServer,
};

use crate::{messages::ServerMessageBuffer, LocalPlayer};

pub fn read_inputs(
    mut input_rollback: ResMut<InputRollback>,
    local_player: Res<LocalPlayer>,
    keyboard_input: Res<Input<KeyCode>>,
    server_messages: Res<ServerMessageBuffer>,
    mut rollback_request: ResMut<RollbackRequest>,
    frame: Res<SyncFrameCount>,
) {
    // Collect local player input.
    let mut input = RawPlayerInput::default();
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

    if input != RawPlayerInput::default() {
        input_rollback.accept_input(IdPlayerInput {
            player_id: local_player.id,
            input: input.at_frame(frame.0),
        });
    }

    for message in server_messages.unreliable.iter() {
        match message {
            UMFromServer::IdPlayerInput(id_player_input) => {
                info!(
                    "Accepting input on frame {}, current frame is {}",
                    id_player_input.input.frame, frame.0
                );
                input_rollback.accept_input(*id_player_input);
                rollback_request.request(id_player_input.input.frame);
            }
            _ => {}
        }
    }
}

pub fn broadcast_local_input(
    input_rollback: Res<InputRollback>,
    local_player: Res<LocalPlayer>,
    mut client: ResMut<RenetClient>,
    frame: Res<SyncFrameCount>,
) {
    let local_input = input_rollback.get_latest().0.get(&local_player.id);
    if let Some(input) = local_input {
        client.send_message(
            DefaultChannel::Unreliable,
            UMFromClient::PlayerInput(input.at_frame(frame.0)),
        );
    }
}

pub fn process_inputs(
    input_rollback: Res<InputRollback>,
    mut players: Query<(&Player, &mut Transform)>,
    time: Res<Time>,
) {
    let players = players
        .iter_mut()
        .map(|(pos, transform)| (pos, transform.into_inner()));

    common::process_input(&input_rollback.get_latest(), players, time.delta_seconds());
}
