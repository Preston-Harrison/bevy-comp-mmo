use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use common::Player;

use crate::LocalPlayer;

pub fn get_player_sprite_bundle(remote: bool) -> SpriteBundle {
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
) {
    for (entity, player) in player_q.iter_mut() {
        commands
            .entity(entity)
            .insert(get_player_sprite_bundle(player.id != local_player.id))
            .insert(Collider::ball(0.5))
            .insert(RigidBody::KinematicPositionBased)
            .insert(KinematicCharacterController::default());
    }
}
