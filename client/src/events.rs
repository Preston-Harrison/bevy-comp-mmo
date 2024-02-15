use bevy::prelude::*;
use bevy_renet::renet::{DefaultChannel, RenetClient};
use common::{bundles::PlayerLogicBundle, Player, PlayerId, PlayerLogin, ROMFromClient, ROMFromServer};

use crate::{spawn::spawn_remote_player, AppState, LocalPlayer};

pub fn handle_login(
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
}

pub fn handle_game_events(
    mut commands: Commands,
    mut client: ResMut<RenetClient>,
    local_player: Res<LocalPlayer>,
    players_q: Query<(Entity, &Player)>,
) {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let event = match ROMFromServer::try_from(message) {
            Ok(event) => event,
            Err(err) => {
                warn!("Failed to deserialize server event: {}", err);
                continue;
            }
        };

        match event {
            ROMFromServer::PlayerConnected(player_id) => {
                if player_id == local_player.id {
                    info!("Spawning local player with id {}", player_id.0);
                    spawn_local_player(&mut commands, player_id);
                } else {
                    info!("Spawning remote player with id {}", player_id.0);
                    spawn_remote_player(&mut commands, player_id, Transform::default());
                }
            }
            ROMFromServer::PlayerDisconnected(player_id) => {
                info!("Despawning remote player with id {}", player_id.0);
                for (entity, player) in players_q.iter() {
                    if player.id == player_id {
                        commands.entity(entity).despawn_recursive();
                    }
                }
            }
        }
    }
}

fn spawn_local_player(commands: &mut Commands, player_id: PlayerId) {
    commands
        .spawn(PlayerLogicBundle::new(player_id))
        .insert(SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.0, 1.0, 0.0),
                custom_size: Some(Vec2::new(30.0, 30.0)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(TransformBundle::default());

    commands.spawn(Camera2dBundle::default());
}
