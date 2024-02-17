use bevy::prelude::*;
use common::{rollback::{RollbackRequest, TransformRollback}, GameSync, PlayerId, ServerEntityMap};

use crate::spawn::get_player_sprite_bundle;

pub fn apply_game_sync(
    commands: &mut Commands,
    transform_rollback: &mut TransformRollback,
    sync: &GameSync,
    server_entity_map: &mut ServerEntityMap,
    local_player_id: PlayerId,
    rollback_request: &mut RollbackRequest,
) {
    info!("Syncing game from frame {}", sync.frame);

    // Can just spawn the player, spawning is not a rollbackable action.
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

    // Cannot just set transform, must set transform rollback and request rollback.
    for (server_obj, transform) in sync.transforms.iter() {
        let eid = match server_entity_map.0.get(server_obj).copied() {
            Some(eid) => eid,
            None => {
                let eid = commands.spawn(*server_obj).id();
                server_entity_map.0.insert(*server_obj, eid);
                eid
            }
        };
        transform_rollback.set_transform_at_frame(eid, *transform, sync.frame);
    }

    rollback_request.request(sync.frame);
}
