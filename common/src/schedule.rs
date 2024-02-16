use bevy::prelude::*;

#[derive(Hash, Debug, PartialEq, Eq, Clone, SystemSet)]
pub enum GameSchedule {
    Init,
    Main,
    Rollback,
}

pub struct GameSchedulePlugin;

impl Plugin for GameSchedulePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                GameSchedule::Init,
                GameSchedule::Main,
                GameSchedule::Rollback,
            )
                .chain(),
        );
    }
}
