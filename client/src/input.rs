use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{
    rollback::{InputRollback, RollbackRequest, SyncFrameCount},
    IdPlayerInput, RawPlayerInput, UMFromClient, UMFromServer,
};

use crate::{messages::ServerMessages, LocalPlayer};

pub fn read_inputs(
    mut input_rollback: ResMut<InputRollback>,
    local_player: Res<LocalPlayer>,
    keyboard_input: Res<Input<KeyCode>>,
    server_messages: ResMut<ServerMessages>,
    mut rollback_request: ResMut<RollbackRequest>,
    frame: Res<SyncFrameCount>,
    mut client: ResMut<RenetClient>,
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
        // @TODO - apply mock input latency.
        client.send_message(
            DefaultChannel::Unreliable,
            UMFromClient::PlayerInput(input),
        );
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
                if id_player_input.input.frame < frame.count() {
                    rollback_request.request(id_player_input.input.frame);
                }
            }
            _ => {}
        }
    }
}
