use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{rollback::InputFrame, Player};

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
            .add_systems(GameLogic, move_player);
    }
}
