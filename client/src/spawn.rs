use bevy::prelude::*;

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
