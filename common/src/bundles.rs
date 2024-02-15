use bevy::prelude::*;

use crate::{Player, PlayerId};

/// Probs will have collider & stuff later.
#[derive(Bundle, Default)]
pub struct PlayerLogicBundle {
    pub player: Player,
    pub transform_bundle: TransformBundle,
}

impl PlayerLogicBundle {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player: Player {
                id: player_id,
                ..Default::default()
            },
            transform_bundle: TransformBundle::default(),
        }
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform_bundle.local = transform;
        self
    }
}
