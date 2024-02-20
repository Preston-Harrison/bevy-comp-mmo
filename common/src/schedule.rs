use bevy::prelude::*;

#[derive(Hash, Debug, PartialEq, Eq, Clone, SystemSet)]
pub enum ServerSchedule {
    InputHandling,
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
    ServerMessages,
    InputCollection,
    EventCollection,
    Rollback,
    UI,
    FrameUpdate,
}

pub struct ClientSchedulePlugin;

impl Plugin for ClientSchedulePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                ClientSchedule::ServerMessages,
                ClientSchedule::InputCollection,
                ClientSchedule::EventCollection,
                ClientSchedule::Rollback,
                ClientSchedule::UI,
                ClientSchedule::FrameUpdate,
            )
                .chain(),
        );
    }
}
