use bevy::{
    ecs::schedule::ScheduleLabel,
    prelude::*,
    transform::systems::{propagate_transforms, sync_simple_transforms},
};
use bevy_rapier2d::{
    control::KinematicCharacterController,
    plugin::{NoUserData, PhysicsSet, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};

use crate::{rollback::InputFrame, Player, FRAME_DURATION_SECONDS};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum GameSet {
    PlayerMovement,
    Physics,
    TransformPropagation,
}

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameLogic;

pub fn move_player(
    mut player_q: Query<(&Player, &mut KinematicCharacterController)>,
    input_frame: Res<InputFrame>,
) {
    for (player, mut controller) in player_q.iter_mut() {
        if let Some(input) = input_frame.get(&player.id) {
            if controller.translation.is_some() {
                warn!("Overwriting translation for player {}", player.id);
            }
            controller.translation = Some(Vec2::new(
                input.x_move as f32 * player.speed * FRAME_DURATION_SECONDS as f32,
                input.y_move as f32 * player.speed * FRAME_DURATION_SECONDS as f32,
            ));

            if input.shoot {
                info!("Player {} is shooting", player.id);
            }
        }
    }
}

pub struct GameLogicPlugin;

impl Plugin for GameLogicPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(GameLogic)
            .configure_sets(
                GameLogic,
                (
                    GameSet::PlayerMovement,
                    GameSet::Physics,
                    GameSet::TransformPropagation,
                )
                    .chain(),
            )
            .add_plugins((
                RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(16.0)
                    .with_default_system_setup(false),
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
                    (sync_simple_transforms, propagate_transforms)
                        .chain()
                        .in_set(GameSet::TransformPropagation),
                ),
            );
    }
}
