use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Player;

/// Probs will have collider & stuff later.
#[derive(Bundle, Copy, Clone, Serialize, Deserialize, Debug)]
pub struct PlayerData {
    pub player: Player,
    pub transform: Transform,
}
