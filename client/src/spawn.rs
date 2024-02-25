use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use common::Player;

use crate::LocalPlayer;

pub fn get_player_sprite(remote: bool) -> SpriteBundle {
    SpriteBundle {
        sprite: Sprite {
            color: if remote {
                Color::rgb(1.0, 0.0, 0.0)
            } else {
                Color::rgb(0.0, 1.0, 0.0)
            },
            custom_size: Some(Vec2::new(30.0, 30.0)),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn attach_player_sprite(
    mut commands: Commands,
    local_player: Res<LocalPlayer>,
    mut player_q: Query<(Entity, &Player), Added<Player>>,
    transform_q: Query<&Transform>,
) {
    for (entity, player) in player_q.iter_mut() {
        info!("Spawning attachments for {}", player.id);
        let transform = transform_q.get(entity).cloned().unwrap_or_default();
        commands
            .entity(entity)
            .insert(get_player_sprite(player.id != local_player.id))
            .insert(transform)
            .insert(Collider::ball(16.0))
            .insert(RigidBody::KinematicPositionBased)
            .insert(KinematicCharacterController::default());
    }
}
