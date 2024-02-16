use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, utils::HashMap};
use bevy_renet::{
    renet::{
        transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
        ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent,
    },
    transport::NetcodeServerPlugin,
    RenetServerPlugin,
};
use common::{
    bundles::PlayerLogicBundle,
    rollback::{InputRollback, RollbackPlugin},
    schedule::{GameSchedule, GameSchedulePlugin},
    FrameCount, GameSync, IdPlayerInput, Player, PlayerId, ROMFromClient, ROMFromServer,
    ServerObject, UMFromClient, UMFromServer,
};
use std::{
    net::UdpSocket,
    time::{Duration, SystemTime},
};

#[derive(Resource, Default)]
struct Clients {
    players: HashMap<ClientId, PlayerId>,
}

#[derive(Resource)]
struct GameSyncTimer(Timer);

impl Default for GameSyncTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(10.0, TimerMode::Repeating))
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
            1.0 / 60.0,
        ))),
    );
    app.insert_resource(common::fixed_timestep_rate());
    app.add_plugins(LogPlugin::default());
    app.add_plugins(RenetServerPlugin);
    app.init_resource::<Clients>();
    app.init_resource::<FrameCount>();
    app.init_resource::<GameSyncTimer>();

    app.add_plugins(GameSchedulePlugin);
    app.add_plugins(RollbackPlugin);

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
            sync_game,
            update_frame_count,
            receive_message_system,
            process_inputs,
            handle_events_system,
        )
            .chain()
            .in_set(GameSchedule::Main),
    );
    app.run();
}

fn update_frame_count(mut frame_count: ResMut<FrameCount>) {
    frame_count.0 += 1;
}

fn sync_game(
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
    mut timer: ResMut<GameSyncTimer>,
    transform_q: Query<(&ServerObject, &Transform)>,
    player_q: Query<(&ServerObject, &Player)>,
    frame_count: Res<FrameCount>,
) {
    timer.0.tick(time.delta());
    if timer.0.finished() {
        info!("Syncing game");
        server.broadcast_message(
            DefaultChannel::Unreliable,
            UMFromServer::GameSync(GameSync {
                transforms: transform_q
                    .iter()
                    .map(|(server_obj, transform)| (*server_obj, *transform))
                    .collect(),
                players: player_q
                    .iter()
                    .map(|(server_obj, player)| (*server_obj, *player))
                    .collect(),
                frame: frame_count.0,
            }),
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
    frame_count: Res<FrameCount>,
) {
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::Unreliable) {
            let Ok(client_message) = UMFromClient::try_from(message) else {
                warn!("Failed to deserialize client event");
                continue;
            };

            match client_message {
                UMFromClient::PlayerInput(framed_input) => {
                    info!("Receiving input {:?}", framed_input);
                    let Some(player_id) = clients.players.get(&client_id) else {
                        warn!("Client {} not logged in", client_id);
                        continue;
                    };
                    let id_input = IdPlayerInput {
                        player_id: *player_id,
                        input: framed_input,
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

            if clients.players.contains_key(&client_id) {
                warn!("Client {} already logged in", client_id);
                continue;
            }
            clients.players.insert(client_id, login.id);
            let entity = PlayerLogicBundle::new(login.id, ServerObject::rand());
            server.send_message(
                client_id,
                DefaultChannel::ReliableOrdered,
                ROMFromServer::GameSync(GameSync {
                    transforms: transform_q
                        .iter()
                        .chain(std::iter::once((
                            &entity.server_object,
                            &entity.transform_bundle.local,
                        )))
                        .map(|(server_obj, transform)| (*server_obj, *transform))
                        .collect(),
                    players: player_q
                        .iter()
                        .chain(std::iter::once((&entity.server_object, &entity.player)))
                        .map(|(server_obj, player)| (*server_obj, *player))
                        .collect(),
                    frame: frame_count.0,
                }),
            );
            server.broadcast_message(
                DefaultChannel::ReliableOrdered,
                ROMFromServer::PlayerConnected {
                    player_id: login.id,
                    server_object: entity.server_object,
                },
            );
            commands.spawn(entity);
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

fn process_inputs(
    input_rollback: Res<InputRollback>,
    mut players: Query<(&Player, &mut Transform)>,
    time: Res<Time>,
) {
    let players = players
        .iter_mut()
        .map(|(pos, transform)| (pos, transform.into_inner()))
        .collect::<Vec<_>>();

    common::process_input(
        input_rollback.get_latest(),
        players.into_iter(),
        time.delta_seconds(),
    );
}
