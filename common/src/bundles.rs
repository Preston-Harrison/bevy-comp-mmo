use bevy::prelude::*;

use crate::{Player, PlayerId, ServerObject};

/// Probs will have collider & stuff later.
#[derive(Bundle)]
pub struct PlayerLogicBundle {
    pub player: Player,
    pub server_object: ServerObject,
    pub transform_bundle: TransformBundle,
}

impl PlayerLogicBundle {
    pub fn new(player_id: PlayerId, server_object: ServerObject) -> Self {
        Self {
            player: Player {
                id: player_id,
                ..Default::default()
            },
            server_object,
            transform_bundle: TransformBundle::default(),
        }
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform_bundle.local = transform;
        self
    }
}
