use bevy::{prelude::*, utils::HashMap};
use bevy_renet::{
    renet::{
        transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
        ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent,
    },
    transport::NetcodeServerPlugin,
    RenetServerPlugin,
};
use common::{
    bundles::PlayerLogicBundle, FrameCount, GameSync, IdPlayerInput, InputBuffer, Player, PlayerId,
    ROMFromClient, ROMFromServer, UMFromClient, UMFromServer,
};
use std::{net::UdpSocket, time::SystemTime};

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
    app.add_plugins(DefaultPlugins);
    app.add_plugins(RenetServerPlugin);
    app.init_resource::<Clients>();
    app.init_resource::<InputBuffer>();
    app.init_resource::<FrameCount>();
    app.init_resource::<GameSyncTimer>();

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
            .chain(),
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
    player_q: Query<(&Player, &Transform)>,
) {
    timer.0.tick(time.delta());
    if timer.0.finished() {
        server.broadcast_message(
            DefaultChannel::Unreliable,
            UMFromServer::GameSync(GameSync {
                players: player_q
                    .iter()
                    .map(|(player, transform)| (player.id, transform.clone()))
                    .collect(),
                frame: 0,
            }),
        );
    }
}

fn receive_message_system(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    mut clients: ResMut<Clients>,
    query: Query<(&Player, &Transform)>,
    mut input_buffer: ResMut<InputBuffer>,
    frame_count: Res<FrameCount>,
) {
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::Unreliable) {
            let Ok(client_message) = UMFromClient::try_from(message) else {
                warn!("Failed to deserialize client event");
                continue;
            };

            match client_message {
                UMFromClient::PlayerInput(player_input) => {
                    let Some(player_id) = clients.players.get(&client_id) else {
                        warn!("Client {} not logged in", client_id);
                        continue;
                    };
                    input_buffer.0.insert(*player_id, player_input);
                    let id_input = IdPlayerInput(*player_id, player_input);
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
            commands.spawn(PlayerLogicBundle::new(login.id));
            server.broadcast_message(
                DefaultChannel::ReliableOrdered,
                ROMFromServer::PlayerConnected(login.id),
            );
            server.send_message(
                client_id,
                DefaultChannel::Unreliable,
                UMFromServer::GameSync(GameSync {
                    players: query
                        .iter()
                        .map(|(player, transform)| (player.id, transform.clone()))
                        .collect(),
                    frame: frame_count.0,
                }),
            );
        }
    }
}

fn handle_events_system(
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
                server.broadcast_message(
                    DefaultChannel::ReliableOrdered,
                    ROMFromServer::PlayerDisconnected(player_id),
                );
            }
        }
    }
}

fn process_inputs(
    mut input_buffer: ResMut<InputBuffer>,
    mut players: Query<(&Player, &mut Transform)>,
    time: Res<Time>,
) {
    let mut players = players
        .iter_mut()
        .map(|(pos, transform)| (pos, transform.into_inner()))
        .collect::<Vec<_>>();
    common::process_input(&input_buffer, &mut players, time.delta_seconds());
    input_buffer.0.clear();
}
