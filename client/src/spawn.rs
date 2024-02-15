use bevy::prelude::*;
use common::{bundles::PlayerLogicBundle, PlayerId};

pub fn spawn_remote_player(commands: &mut Commands, player_id: PlayerId, transform: Transform) {
    commands
        .spawn(PlayerLogicBundle::new(player_id))
        .insert(SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(1.0, 0.0, 0.0),
                custom_size: Some(Vec2::new(30.0, 30.0)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(TransformBundle::from_transform(transform));
}