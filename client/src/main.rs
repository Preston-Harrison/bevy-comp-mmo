use bevy::prelude::*;
use bevy_renet::{
    renet::{
        transport::{ClientAuthentication, NetcodeClientTransport},
        ConnectionConfig, RenetClient,
    },
    transport::NetcodeClientPlugin,
    RenetClientPlugin,
};
use clap::Parser;
use common::{
    game::GameLogicPlugin,
    rollback::RollbackPluginClient,
    schedule::{ClientSchedule, ClientSchedulePlugin, ClientState},
    PlayerId, ServerEntityMap,
};
use events::{handle_login, send_login};
use messages::{ServerMessages, ServerMessageBuffer};
use spawn::attach_player_sprite;
use std::{net::UdpSocket, sync::OnceLock, time::SystemTime};
use ui::UIPlugin;

mod events;
mod input;
mod messages;
mod spawn;
mod ui;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = 0)]
    id: u64,

    /// Mocked extra latency in milliseconds.
    #[arg(short, long, default_value_t = 0.0)]
    network_latency: f32,
}

static ARGS: OnceLock<Args> = OnceLock::new();

fn main() {
    ARGS.get_or_init(|| Args::parse());

    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_state::<ClientState>()
        .add_plugins(ClientSchedulePlugin)
        .add_plugins(RollbackPluginClient)
        .add_plugins(GameLogicPlugin)
        .add_plugins(UIPlugin)
        .add_systems(Startup, send_login)
        .add_systems(
            FixedUpdate,
            handle_login.run_if(in_state(ClientState::MainMenu)),
        )
        .add_systems(
            FixedUpdate,
            messages::receive_messages.in_set(ClientSchedule::ServerMessageCollection),
        )
        .add_systems(
            FixedUpdate,
            (
                input::read_inputs.in_set(ClientSchedule::InputCollection),
                events::handle_game_events.in_set(ClientSchedule::ServerEventHandling),
            )
                .run_if(in_state(ClientState::InGame)),
        )
        .add_systems(
            FixedUpdate,
            attach_player_sprite
                .in_set(ClientSchedule::ServerReactive)
                .run_if(in_state(ClientState::InGame)),
        )
        .init_resource::<ServerMessages>()
        // @TODO fix this
        .init_resource::<ServerMessageBuffer>()
        .init_resource::<ServerEntityMap>()
        .insert_resource(common::fixed_timestep_rate())
        .insert_resource(LocalPlayer {
            id: PlayerId(ARGS.get().unwrap().id),
        });

    app.add_plugins(RenetClientPlugin);

    let client = RenetClient::new(ConnectionConfig::default());
    app.insert_resource(client);

    // Setup the transport layer
    app.add_plugins(NetcodeClientPlugin);

    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let authentication = ClientAuthentication::Unsecure {
        server_addr,
        client_id: ARGS.get().unwrap().id,
        user_data: None,
        protocol_id: 0,
    };
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

    app.insert_resource(transport);
    app.run();
}

#[derive(Resource)]
struct LocalPlayer {
    id: PlayerId,
}
