use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{
    rollback::{InputRollback, RollbackRequest, SyncFrameCount},
    IdPlayerInput, RawPlayerInput, UMFromClient, UMFromServer,
};

use crate::{messages::ServerMessageBuffer, LocalPlayer};

pub fn read_inputs(
    mut input_rollback: ResMut<InputRollback>,
    local_player: Res<LocalPlayer>,
    keyboard_input: Res<Input<KeyCode>>,
    server_messages: ResMut<ServerMessageBuffer>,
    mut rollback_request: ResMut<RollbackRequest>,
    frame: Res<SyncFrameCount>,
) {
    let mut had_input = false;

    // Collect local player input.
    let mut input = RawPlayerInput::default();
    if keyboard_input.pressed(KeyCode::W) {
        input.y_move += 1;
        had_input = true;
    }
    if keyboard_input.pressed(KeyCode::S) {
        input.y_move -= 1;
        had_input = true;
    }
    if keyboard_input.pressed(KeyCode::A) {
        input.x_move -= 1;
        had_input = true;
    }
    if keyboard_input.pressed(KeyCode::D) {
        input.x_move += 1;
        had_input = true;
    }
    if keyboard_input.pressed(KeyCode::Space) {
        input.shoot = true;
        had_input = true;
    }

    if had_input {
        input_rollback.accept_input(IdPlayerInput {
            player_id: local_player.id,
            input: input.at_frame(frame.count()),
        });
    }

    for message in server_messages.unreliable.iter() {
        match message {
            UMFromServer::IdPlayerInput(id_player_input) => {
                info!(
                    "Accepting input on frame {} from {}, current frame is {}",
                    id_player_input.input.frame,
                    id_player_input.player_id,
                    frame.count()
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
) {
    let local_input = input_rollback
        .get_latest()
        .and_then(|x| x.get(&local_player.id));
    if let Some(input) = local_input {
        client.send_message(
            DefaultChannel::Unreliable,
            UMFromClient::PlayerInput(*input),
        );
    }
}
