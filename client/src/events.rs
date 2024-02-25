use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{
    rollback::{
        ComponentRollbacks, GameSyncRequest, InputRollback, RollbackRequest, SyncFrameCount,
    },
    schedule::ClientState,
    Player, PlayerLogin, ROMFromClient, ROMFromServer, ServerEntityMap, ServerObject, UMFromServer,
};

use crate::{messages::ServerMessageBuffer, spawn::get_player_sprite, LocalPlayer};

pub fn send_login(mut client: ResMut<RenetClient>, local_player: Res<LocalPlayer>) {
    info!("Sending login");
    client.send_message(
        DefaultChannel::ReliableOrdered,
        ROMFromClient::PlayerLogin(PlayerLogin {
            id: local_player.id,
        }),
    );
}

pub fn handle_login(
    mut commands: Commands,
    mut next_state: ResMut<NextState<ClientState>>,
    server_messages: Res<ServerMessageBuffer>,
) {
    info!("Checking for login initial sync");
    for message in server_messages.reliable_ordered.iter() {
        match message {
            ROMFromServer::GameSync(game_sync) => {
                info!("Initial game sync {:?}", game_sync);
                // Add one to initial frame to account for the frame we are currently on.
                let init_frame =
                    game_sync.frame + common::frames_since_unix_time(game_sync.unix_time) + 1;
                info!("Starting game from frame: {}", init_frame);

                commands.insert_resource(SyncFrameCount::new(init_frame));
                commands.insert_resource(ComponentRollbacks::from_frame(init_frame - 1));
                commands.insert_resource(GameSyncRequest::new(game_sync.clone()));
                commands.insert_resource(RollbackRequest::default());
                commands.insert_resource(InputRollback::from_frame(init_frame));

                commands.spawn(Camera2dBundle::default());
                next_state.set(ClientState::InGame);
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
    mut game_sync_req: ResMut<GameSyncRequest>,
) {
    for message in server_messages.reliable_ordered.iter() {
        match message {
            ROMFromServer::PlayerConnected {
                player_data,
                server_object,
            } => {
                if player_data.player.id != local_player.id {
                    info!("Spawning remote player with id {}", player_data.player.id.0);
                    let eid = commands
                        .spawn(*server_object)
                        .insert(player_data.clone())
                        .insert(get_player_sprite(true))
                        .id();
                    server_entity_map.insert(*server_object, eid).unwrap();
                }
            }
            ROMFromServer::PlayerDisconnected(player_id) => {
                info!("Despawning remote player with id {}", player_id.0);
                for (entity, player, server_object) in player_q.iter() {
                    if &player.id == player_id {
                        commands.entity(entity).despawn_recursive();
                        server_entity_map.remove(server_object);
                    }
                }
            }
            ROMFromServer::GameSync(_) => {}
        }
    }

    for message in server_messages.unreliable.iter() {
        match message {
            UMFromServer::GameSync(game_sync) => {
                game_sync_req.request(game_sync.clone());
                info!("Receving sync for frame {}", game_sync.frame);
            }
            _ => {}
        }
    }
}
