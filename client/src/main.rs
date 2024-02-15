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
use common::{InputBuffer, PlayerId};
use events::handle_login;
use std::{net::UdpSocket, time::SystemTime};

mod events;
mod input;
mod rollback;
mod spawn;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = 0)]
    id: u64,
}

#[derive(States, Default, Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    #[default]
    MainMenu,
    InGame,
}

fn main() {
    let args = Args::parse();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_state::<AppState>()
        .add_systems(Startup, handle_login.run_if(in_state(AppState::MainMenu)))
        .add_systems(
            FixedUpdate,
            (
                events::handle_game_events,
                input::read_inputs,
                input::broadcast_local_input,
                input::process_inputs,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        )
        .init_resource::<InputBuffer>()
        .insert_resource(LocalPlayer {
            id: PlayerId(args.id),
        });

    app.add_plugins(RenetClientPlugin);

    let client = RenetClient::new(ConnectionConfig::default());
    app.insert_resource(client);

    // Setup the transport layer
    app.add_plugins(NetcodeClientPlugin);

    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let authentication = ClientAuthentication::Unsecure {
        server_addr,
        client_id: args.id,
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
