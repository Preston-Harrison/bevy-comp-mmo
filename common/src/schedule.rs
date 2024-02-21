use bevy::prelude::*;

#[derive(Hash, Debug, PartialEq, Eq, Clone, SystemSet)]
pub enum ServerSchedule {
    InputHandling,
    Connections,
    Rollback,
    GameSync,
    Debug,
    FrameUpdate,
}

pub struct ServerSchedulePlugin;

impl Plugin for ServerSchedulePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                ServerSchedule::InputHandling,
                ServerSchedule::Connections,
                ServerSchedule::Rollback,
                ServerSchedule::GameSync,
                ServerSchedule::Debug,
                ServerSchedule::FrameUpdate,
            )
                .chain(),
        );
    }
}

#[derive(Hash, Debug, PartialEq, Eq, Clone, SystemSet)]
pub enum ClientSchedule {
    ServerMessageCollection,
    InputCollection,
    ServerEventHandling,
    Rollback,
    ServerReactive,
    FrameUpdate,
}

pub struct ClientSchedulePlugin;

impl Plugin for ClientSchedulePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                ClientSchedule::ServerMessageCollection,
                ClientSchedule::InputCollection,
                ClientSchedule::ServerEventHandling,
                ClientSchedule::Rollback,
                ClientSchedule::ServerReactive,
                ClientSchedule::FrameUpdate,
            )
                .chain(),
        );
    }
}

#[derive(States, Default, Debug, Clone, Eq, PartialEq, Hash)]
pub enum ClientState {
    #[default]
    MainMenu,
    InGame,
}
