use bevy::prelude::*;
use common::{GameSync, Player};

use crate::spawn::spawn_remote_player;

pub fn apply_game_sync(commands: &mut Commands, sync: GameSync, players_q: &[(Entity, &Player)]) {
    for (player_id, transform) in sync.players {
        let Some((entity, _)) = players_q.iter().find(|(_, p)| p.id == player_id) else {
            spawn_remote_player(commands, player_id, transform);
            continue;
        };
        commands.entity(*entity).insert(transform);
    }
}
