use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{
    bundles::PlayerLogicBundle,
    rollback::{RollbackRequest, SyncFrameCount, TransformRollback},
    Player, PlayerLogin, ROMFromClient, ROMFromServer, ServerEntityMap, ServerObject, UMFromServer,
};

use crate::{
    game_sync::apply_game_sync, messages::ServerMessageBuffer, spawn::get_player_sprite_bundle,
    AppState, LocalPlayer,
};

pub fn send_login(mut client: ResMut<RenetClient>, local_player: Res<LocalPlayer>) {
    client.send_message(
        DefaultChannel::ReliableOrdered,
        ROMFromClient::PlayerLogin(PlayerLogin {
            id: local_player.id,
        }),
    );
}

pub fn handle_login(
    mut commands: Commands,
    local_player: Res<LocalPlayer>,
    mut next_state: ResMut<NextState<AppState>>,
    mut server_entity_map: ResMut<ServerEntityMap>,
    server_messages: Res<ServerMessageBuffer>,
    mut rollback_request: ResMut<RollbackRequest>,
    mut frame: ResMut<SyncFrameCount>,
    mut transform_rollback: ResMut<TransformRollback>,
) {
    for message in server_messages.reliable_ordered.iter() {
        match message {
            ROMFromServer::GameSync(game_sync) => {
                info!("Initial game sync");
                let init_frame =
                    game_sync.frame + common::frames_since_unix_time(game_sync.unix_time);
                frame.0 = init_frame;
                info!("Starting game from frame: {}", init_frame);

                apply_game_sync(
                    &mut commands,
                    &mut transform_rollback,
                    game_sync,
                    &mut server_entity_map,
                    local_player.id,
                    &mut rollback_request,
                );

                commands.spawn(Camera2dBundle::default());
                next_state.set(AppState::InGame);
            }
            _ => {}
        }
    }
}

pub fn handle_game_events(
    mut commands: Commands,
    server_messages: Res<ServerMessageBuffer>,
    local_player: Res<LocalPlayer>,
    mut server_entity_map: ResMut<ServerEntityMap>,
    player_q: Query<(Entity, &Player, &ServerObject)>,
    mut rollback_request: ResMut<RollbackRequest>,
    mut transform_rollback: ResMut<TransformRollback>,
) {
    for message in server_messages.reliable_ordered.iter() {
        match message {
            ROMFromServer::PlayerConnected {
                player_id,
                server_object,
            } => {
                if player_id != &local_player.id {
                    info!("Spawning remote player with id {}", player_id.0);
                    let eid = commands
                        .spawn(PlayerLogicBundle::new(*player_id, *server_object))
                        .insert(get_player_sprite_bundle(true))
                        .id();
                    server_entity_map.0.insert(*server_object, eid);
                }
            }
            ROMFromServer::PlayerDisconnected(player_id) => {
                info!("Despawning remote player with id {}", player_id.0);
                for (entity, player, server_object) in player_q.iter() {
                    if &player.id == player_id {
                        commands.entity(entity).despawn_recursive();
                        server_entity_map.0.remove(server_object);
                    }
                }
            }
            ROMFromServer::GameSync(_) => {}
        }
    }

    for message in server_messages.unreliable.iter() {
        match message {
            UMFromServer::GameSync(game_sync) => {
                apply_game_sync(
                    &mut commands,
                    &mut transform_rollback,
                    game_sync,
                    &mut server_entity_map,
                    local_player.id,
                    &mut rollback_request,
                );
                info!("Receving sync for frame {}", game_sync.frame);
            }
            _ => {}
        }
    }
}
