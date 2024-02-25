use bevy::{prelude::*, utils::HashMap};
use bevy_rapier2d::prelude::*;
use bevy_renet::{
    renet::{
        transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
        ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent,
    },
    transport::NetcodeServerPlugin,
    RenetServerPlugin,
};
use common::{
    bundles::PlayerData,
    game::GameLogicPlugin,
    rollback::{InputRollback, RollbackPluginServer, SyncFrameCount},
    schedule::{ServerSchedule, ServerSchedulePlugin},
    GameSync, IdPlayerInput, Player, PlayerId, ROMFromClient, ROMFromServer, ServerObject,
    UMFromClient, UMFromServer,
};
use std::{net::UdpSocket, time::SystemTime};

#[cfg(feature = "debug")]
mod ui;

#[derive(Resource, Default)]
struct Clients {
    players: HashMap<ClientId, PlayerId>,
}

#[derive(Resource)]
struct GameSyncTimer(Timer);

impl Default for GameSyncTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(3.0, TimerMode::Repeating))
    }
}

fn main() {
    let mut app = App::new();

    #[cfg(not(feature = "debug"))]
    {
        use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin};
        use std::time::Duration;

        app.add_plugins(LogPlugin::default());
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f32(common::FRAME_DURATION_SECONDS as f32),
        )));
    }

    #[cfg(feature = "debug")]
    {
        app.add_plugins(DefaultPlugins);
    }

    app.insert_resource(common::fixed_timestep_rate());
    app.add_plugins(RenetServerPlugin);
    app.init_resource::<Clients>();
    app.init_resource::<GameSyncTimer>();

    #[cfg(feature = "debug")]
    app.add_plugins(ui::UIPlugin);

    app.add_plugins(ServerSchedulePlugin);
    app.add_plugins(RollbackPluginServer);
    app.add_plugins(GameLogicPlugin);

    let server = RenetServer::new(ConnectionConfig::default());
    app.insert_resource(server);

    // Transport layer setup
    app.add_plugins(NetcodeServerPlugin);
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let socket = UdpSocket::bind(server_addr).unwrap();
    let server_config = ServerConfig {
        current_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap(),
        max_clients: 64,
        protocol_id: 0,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Unsecure,
    };
    let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
    app.insert_resource(transport);

    app.add_systems(
        FixedUpdate,
        (
            receive_message_system.in_set(ServerSchedule::InputHandling),
            handle_events_system.in_set(ServerSchedule::Connections),
            sync_game.in_set(ServerSchedule::GameSync),
        ),
    );
    app.run();
}

fn sync_game(
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
    mut timer: ResMut<GameSyncTimer>,
    transform_q: Query<(&ServerObject, &Transform)>,
    player_q: Query<(&ServerObject, &Player)>,
    frame_count: Res<SyncFrameCount>,
) {
    timer.0.tick(time.delta());
    if timer.0.finished() {
        info!("Syncing game on frame {}", frame_count.count());
        let game_sync = GameSync {
            transforms: transform_q
                .iter()
                .map(|(server_obj, transform)| (*server_obj, *transform))
                .collect(),
            players: player_q
                .iter()
                .map(|(server_obj, player)| (*server_obj, *player))
                .collect(),
            frame: frame_count.count(),
            unix_time: common::get_unix_time(),
        };
        info!("{:?}", game_sync);
        server.broadcast_message(
            DefaultChannel::Unreliable,
            UMFromServer::GameSync(game_sync),
        );
    }
}

fn receive_message_system(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    mut clients: ResMut<Clients>,
    transform_q: Query<(&ServerObject, &Transform)>,
    player_q: Query<(&ServerObject, &Player)>,
    mut input_rollback: ResMut<InputRollback>,
    frame_count: Res<SyncFrameCount>,
    #[cfg(feature = "debug")] mut input_tracker: ResMut<self::ui::InputTracker>,
) {
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::Unreliable) {
            let Ok(client_message) = UMFromClient::try_from(message) else {
                warn!("Failed to deserialize client event");
                continue;
            };

            match client_message {
                UMFromClient::PlayerInput(raw_input) => {
                    let Some(player_id) = clients.players.get(&client_id) else {
                        warn!("Client {} not logged in", client_id);
                        continue;
                    };
                    info!("Accepting input");

                    #[cfg(feature = "debug")]
                    input_tracker
                        .inputs
                        .entry(*player_id)
                        .and_modify(|e| *e += 1)
                        .or_insert(1);

                    let id_input = IdPlayerInput {
                        player_id: *player_id,
                        input: raw_input.at_frame(frame_count.count()),
                    };
                    input_rollback.accept_input(id_input);
                    server.broadcast_message_except(
                        client_id,
                        DefaultChannel::Unreliable,
                        UMFromServer::IdPlayerInput(id_input),
                    );
                }
            }
        }

        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered)
        {
            let Ok(client_message) = ROMFromClient::try_from(message) else {
                warn!("Failed to deserialize client event");
                continue;
            };

            #[allow(irrefutable_let_patterns)] // there will be more.
            let ROMFromClient::PlayerLogin(login) = client_message
            else {
                warn!("Unexpected client message");
                continue;
            };

            info!("Player trying to login");

            if clients.players.contains_key(&client_id) {
                warn!("Client {} already logged in", client_id);
                continue;
            }
            clients.players.insert(client_id, login.id);

            let server_object = ServerObject::rand();
            let player_data = PlayerData {
                player: Player {
                    id: login.id,
                    ..Default::default()
                },
                transform: Transform::default(),
            };

            info!("Sending connection game sync");
            server.send_message(
                client_id,
                DefaultChannel::ReliableOrdered,
                ROMFromServer::GameSync(GameSync {
                    transforms: transform_q
                        .iter()
                        .chain(std::iter::once((&server_object, &player_data.transform)))
                        .map(|(server_obj, transform)| (*server_obj, *transform))
                        .collect(),
                    players: player_q
                        .iter()
                        .chain(std::iter::once((&server_object, &player_data.player)))
                        .map(|(server_obj, player)| (*server_obj, *player))
                        .collect(),
                    frame: frame_count.count() - 1,
                    unix_time: common::get_unix_time(),
                }),
            );
            server.broadcast_message(
                DefaultChannel::ReliableOrdered,
                ROMFromServer::PlayerConnected {
                    player_data,
                    server_object,
                },
            );
            commands
                .spawn(server_object)
                .insert(player_data)
                .insert((
                    Collider::ball(16.0),
                    RigidBody::KinematicPositionBased,
                    KinematicCharacterController::default(),
                ))
                .insert(SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgb(1.0, 0.0, 0.0),
                        custom_size: Some(Vec2::new(30.0, 30.0)),
                        ..Default::default()
                    },
                    ..Default::default()
                });
        }
    }
}

fn handle_events_system(
    mut commands: Commands,
    player_q: Query<(Entity, &Player)>,
    mut server_events: EventReader<ServerEvent>,
    mut server: ResMut<RenetServer>,
    mut clients: ResMut<Clients>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!("Client {client_id} connected");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!("Client {client_id} disconnected: {reason}");
                let Some(player_id) = clients.players.remove(client_id) else {
                    continue;
                };
                for (entity, player) in player_q.iter() {
                    if player.id == player_id {
                        commands.entity(entity).despawn_recursive();
                    }
                }
                server.broadcast_message(
                    DefaultChannel::ReliableOrdered,
                    ROMFromServer::PlayerDisconnected(player_id),
                );
            }
        }
    }
}
