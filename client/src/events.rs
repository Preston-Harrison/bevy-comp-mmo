use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{
    bundles::PlayerLogicBundle, Player, PlayerLogin, ROMFromClient, ROMFromServer, ServerEntityMap,
    ServerObject, UMFromServer,
};

use crate::{
    messages::ServerMessageBuffer, rollback::apply_game_sync, spawn::get_player_sprite_bundle,
    AppState, LocalPlayer,
};

pub fn handle_login(
    mut commands: Commands,
    mut client: ResMut<RenetClient>,
    local_player: Res<LocalPlayer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    client.send_message(
        DefaultChannel::ReliableOrdered,
        ROMFromClient::PlayerLogin(PlayerLogin {
            id: local_player.id,
        }),
    );
    next_state.set(AppState::InGame);
    commands.spawn(Camera2dBundle::default());
}

pub fn handle_game_events(
    mut commands: Commands,
    server_messages: Res<ServerMessageBuffer>,
    local_player: Res<LocalPlayer>,
    mut server_entity_map: ResMut<ServerEntityMap>,
    player_q: Query<(Entity, &Player, &ServerObject)>,
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
            ROMFromServer::GameSync(game_sync) => {
                apply_game_sync(
                    &mut commands,
                    game_sync,
                    &mut server_entity_map,
                    local_player.id,
                );
            }
        }
    }

    for message in server_messages.unreliable.iter() {
        match message {
            UMFromServer::GameSync(game_sync) => {
                apply_game_sync(
                    &mut commands,
                    game_sync,
                    &mut server_entity_map,
                    local_player.id,
                );
            }
            _ => {}
        }
    }
}
