use bevy::prelude::*;
use common::{GameSync, PlayerId, ServerEntityMap};

use crate::spawn::get_player_sprite_bundle;

pub fn apply_game_sync(
    commands: &mut Commands,
    sync: &GameSync,
    server_entity_map: &mut ServerEntityMap,
    local_player_id: PlayerId,
) {
    info!("Syncing game from frame {}", sync.frame);
    for (server_obj, player) in sync.players.iter() {
        let eid = server_entity_map.0.get(server_obj).copied();
        let mut entity = match eid {
            Some(eid) => commands.entity(eid),
            None => {
                let mut entity = commands.spawn(*server_obj);
                server_entity_map.0.insert(*server_obj, entity.id());
                entity.insert(get_player_sprite_bundle(player.id == local_player_id));
                entity
            }
        };
        entity.insert(*player);
    }

    for (server_obj, transform) in sync.transforms.iter() {
        let eid = match server_entity_map.0.get(server_obj).copied() {
            Some(eid) => eid,
            None => {
                let eid = commands.spawn(*server_obj).id();
                server_entity_map.0.insert(*server_obj, eid);
                eid
            }
        };
        commands.entity(eid).insert(*transform);
    }
}
