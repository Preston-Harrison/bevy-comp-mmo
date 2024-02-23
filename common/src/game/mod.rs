use bevy::{ecs::schedule::ScheduleLabel, prelude::*};
use bevy_rapier2d::{
    plugin::{NoUserData, PhysicsSet, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};

use crate::{rollback::InputFrame, Player};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum GameSet {
    PlayerMovement,
    Physics,
}

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameLogic;

pub fn move_player(mut player_q: Query<(&Player, &mut Transform)>, input_frame: Res<InputFrame>) {
    let delta_time = super::FRAME_DURATION_SECONDS as f32;
    for (player, mut transform) in player_q.iter_mut() {
        if let Some(input) = input_frame.get(&player.id) {
            transform.translation.x += input.x as f32 * player.speed * delta_time;
            transform.translation.y += input.y as f32 * player.speed * delta_time;
        }
    }
}

pub struct GameLogicPlugin;

impl Plugin for GameLogicPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(GameLogic)
            .add_plugins((
                RapierPhysicsPlugin::<NoUserData>::default().with_default_system_setup(false),
                RapierDebugRenderPlugin::default(),
            ))
            .add_systems(GameLogic, move_player.in_set(GameSet::PlayerMovement))
            .configure_sets(
                GameLogic,
                (
                    PhysicsSet::SyncBackend,
                    PhysicsSet::StepSimulation,
                    PhysicsSet::Writeback,
                )
                    .chain()
                    .in_set(GameSet::Physics),
            )
            .add_systems(
                GameLogic,
                (
                    RapierPhysicsPlugin::<NoUserData>::get_systems(PhysicsSet::SyncBackend)
                        .in_set(PhysicsSet::SyncBackend),
                    RapierPhysicsPlugin::<NoUserData>::get_systems(PhysicsSet::StepSimulation)
                        .in_set(PhysicsSet::StepSimulation),
                    RapierPhysicsPlugin::<NoUserData>::get_systems(PhysicsSet::Writeback)
                        .in_set(PhysicsSet::Writeback),
                ),
            );
    }
}
