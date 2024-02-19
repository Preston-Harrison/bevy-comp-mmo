/// First element rollback deques is the transform in the current frame. This is reset in `Rollback::Init`.
/// Rollbacks are only valid after all local and remote input collection and game syncs.
use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, hash::Hash};

use crate::{
    impl_inner, is_server, schedule::GameSchedule, GameSync, IdPlayerInput, InputBuffer, Player,
    PlayerId, RawPlayerInput, ServerObject,
};

#[derive(Resource, Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SyncFrameCount(pub u64);
impl_inner!(SyncFrameCount, u64);

pub const ROLLBACK_WINDOW: usize = 10;

#[derive(Debug, Resource)]
pub struct RollbackTracker<K: Eq + Hash, V> {
    /// Front element is the current frame.
    history: VecDeque<HashMap<K, V>>,
    current_frame: u64,
}

impl<K: Eq + Hash, V> RollbackTracker<K, V> {
    pub fn new(current_frame: u64) -> Self {
        let mut history = VecDeque::with_capacity(ROLLBACK_WINDOW);
        history.push_front(HashMap::default());
        Self {
            history,
            current_frame,
        }
    }

    fn init_current_frame(&mut self, current_frame: u64) {
        assert_eq!(
            current_frame - 1,
            self.current_frame,
            "Skipped rollback frame"
        );
        self.current_frame = current_frame;

        self.history.push_front(HashMap::default());
        if self.history.len() > ROLLBACK_WINDOW {
            self.history.pop_back();
        }
    }

    fn get_n_frames_ago(&self, n_frames: u64) -> Option<&HashMap<K, V>> {
        self.history.get(n_frames as usize)
    }

    fn get_latest(&self) -> Option<&HashMap<K, V>> {
        self.get_n_frames_ago(0)
    }

    fn rollback_n_frames(&mut self, frames: u64) {
        for _ in 0..frames {
            self.history.pop_front();
        }
        self.current_frame = self.current_frame.saturating_sub(frames);
    }

    fn set_value_at_frame(&mut self, key: K, value: V, frame: u64) {
        assert!(
            self.current_frame >= frame,
            "Cannot set component in the future"
        );
        self.history
            .get_mut((self.current_frame - frame) as usize)
            .and_then(|map| map.insert(key, value));
    }
}

type TransformRollback = RollbackTracker<Entity, Transform>;
type InputRollback = RollbackTracker<PlayerId, RawPlayerInput>;

#[derive(Resource, Default)]
pub struct RollbackRequest(Option<u64>);

impl RollbackRequest {
    pub fn request(&mut self, rollback_to_frame: u64) {
        if let Some(current_frame) = self.0 {
            self.0 = Some(rollback_to_frame.min(current_frame));
        } else {
            self.0 = Some(rollback_to_frame);
        }
    }
}

#[derive(Resource, Default)]
pub struct GameSyncRequest(Option<GameSync>);

impl GameSyncRequest {
    pub fn request(&mut self, game_sync: GameSync) {
        // TODO - check if game sync is recent.
        self.0 = Some(game_sync);
    }
}

fn handle_rollback() {
    // If game sync and not server (as there is no game sync on server):
    // - Roll back all components to the game sync frame.
    // - Update most recent frame info (which is now rolled back to the game sync frame)

    // Else if rollback request:
    // - Roll back all components to the requested frame.
    // - Resimulate game from the requested frame to the most recent frame.

    // Else just simulate current frame.

    // Track new component values in the current frame.
}

/// Rollback plugin:
/// - Update frame count
/// - Initialize input tracker
/// - Collect inputs and game sync, and record rollback requests and game sync requests
/// - Handle rollback requests and game sync requests with `handle_rollback`
pub struct RollbackPlugin;
