/// First element rollback deques is the transform in the current frame. This is reset in `Rollback::Init`.
/// Rollbacks are only valid after all local and remote input collection and game syncs.
use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, hash::Hash};

use crate::{
    game::GameLogic, impl_inner, schedule::ClientSchedule, GameSync, IdPlayerInput, Player,
    PlayerId, RawPlayerInput, ServerEntityMap,
};

pub mod time;

#[derive(Resource, Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SyncFrameCount(pub u64);
impl_inner!(SyncFrameCount, u64);

impl SyncFrameCount {
    fn increment(&mut self) {
        self.0 += 1;
    }
}

pub const DEFAULT_ROLLBACK_WINDOW: usize = 10;

#[derive(Debug, Resource)]
pub struct RollbackTracker<K: Eq + Hash, V> {
    /// Front element is the current frame.
    history: VecDeque<HashMap<K, V>>,
    current_frame: u64,
    rollback_window: usize,
}

impl<K: Eq + Hash, V> RollbackTracker<K, V> {
    pub fn new(current_frame: u64, rollback_window: usize) -> Self {
        let mut history = VecDeque::with_capacity(rollback_window);
        history.push_front(HashMap::default());
        Self {
            history,
            current_frame,
            rollback_window,
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
        if self.history.len() > self.rollback_window {
            self.history.pop_back();
        }
    }

    fn get_n_frames_ago(&self, n_frames: u64) -> Option<&HashMap<K, V>> {
        self.history.get(n_frames as usize)
    }

    pub fn get_latest(&self) -> Option<&HashMap<K, V>> {
        self.get_n_frames_ago(0)
    }

    pub fn get_rollback_window(&self) -> usize {
        self.rollback_window
    }

    fn delete_n_frames(&mut self, frames: u64) {
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

trait ComponentRollback: Sync + Send {
    fn new_frame_from_world(&mut self, world: &mut World);

    fn rollback_and_sync(&mut self, world: &mut World, game_sync: &GameSync);

    fn rollback_and_update_world(&mut self, frames: u64, world: &mut World);
}

impl<T: Component + Clone> ComponentRollback for RollbackTracker<Entity, T> {
    fn new_frame_from_world(&mut self, world: &mut World) {
        let mut query = world.query::<(Entity, &T)>();
        let current_frame = world.get_resource::<SyncFrameCount>().unwrap().0;
        self.init_current_frame(current_frame);

        for (entity, component) in query.iter(world) {
            self.set_value_at_frame(entity, component.clone(), current_frame);
        }
    }

    /// Syncs `T` in world to current game sync values, rolls back history to game sync frame
    /// and sets the current history frame to the game sync frame.
    fn rollback_and_sync(&mut self, world: &mut World, game_sync: &GameSync) {
        // @FIXME should use rollback_and_update_world since game syncs may not include all entities.
        self.delete_n_frames(self.current_frame - game_sync.frame + 1);
        self.init_current_frame(game_sync.frame);

        let mut se_map = world.remove_resource::<ServerEntityMap>().unwrap();

        for (server_obj, component) in game_sync.get::<T>().iter() {
            let entity = match se_map.get(server_obj) {
                Some(entity) => *entity,
                None => {
                    let entity = world.spawn(*server_obj).id();
                    se_map.insert(*server_obj, entity).unwrap();
                    entity
                }
            };
            world.entity_mut(entity).insert(component.clone());
            self.set_value_at_frame(entity, component.clone(), game_sync.frame);
        }

        world.insert_resource(se_map);
    }

    fn rollback_and_update_world(&mut self, frames: u64, world: &mut World) {
        self.delete_n_frames(frames);
        self.init_current_frame(self.current_frame + 1);

        // @FIXME handle things spawned after the rollback frame
        let Some(frame_values) = self.get_latest() else {
            return;
        };

        for (entity, component) in frame_values.iter() {
            // @FIXME this can happen with despawns, need to soft delete and hard delete after rollback window has elapsed.
            let mut entity = world
                .get_entity_mut(*entity)
                .expect("Entity in rollback does not exist");
            entity.insert(component.clone());
        }
    }
}

pub type TransformRollback = RollbackTracker<Entity, Transform>;
pub type PlayerRollback = RollbackTracker<Entity, Player>;

#[derive(Resource)]
pub struct ComponentRollbacks(Vec<Box<dyn ComponentRollback>>);

impl ComponentRollbacks {
    pub fn new(frame: u64) -> Self {
        Self(vec![
            Box::new(TransformRollback::new(frame, DEFAULT_ROLLBACK_WINDOW)),
            Box::new(PlayerRollback::new(frame, DEFAULT_ROLLBACK_WINDOW)),
        ])
    }
}

pub type InputRollback = RollbackTracker<PlayerId, RawPlayerInput>;

impl InputRollback {
    pub fn accept_input(&mut self, input: IdPlayerInput) {
        self.set_value_at_frame(input.player_id, input.input.raw, input.input.frame);
    }
}

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

fn handle_rollback(world: &mut World) {
    // Game sync resource may not exist here, as it does not exist on the server.
    let game_sync_request = world
        .get_resource_mut::<GameSyncRequest>()
        .and_then(|mut x| x.0.take());
    let rollback_request = world
        .get_resource_mut::<RollbackRequest>()
        .unwrap()
        .0
        .take();
    let frame_count = world.get_resource::<SyncFrameCount>().unwrap().0;

    // Pop transform out of world so it can be edited mutably alongside world.
    let mut component_rollbacks = world.remove_resource::<ComponentRollbacks>().unwrap();

    // @FIXME: could be alot of off by one errors here.
    if let Some(game_sync) = game_sync_request {
        let rollback_count = frame_count - game_sync.frame;

        for rollback in component_rollbacks.0.iter_mut() {
            rollback.rollback_and_sync(world, &game_sync);
        }

        for _ in 0..rollback_count {
            world.run_schedule(GameLogic);
            for rollback in component_rollbacks.0.iter_mut() {
                rollback.new_frame_from_world(world);
            }
        }
    } else if let Some(rollback_frame) = rollback_request {
        let rollback_count = frame_count - rollback_frame;
        for rollback in component_rollbacks.0.iter_mut() {
            rollback.rollback_and_update_world(rollback_count, world);
        }

        for _ in 0..rollback_count {
            world.run_schedule(GameLogic);
            for rollback in component_rollbacks.0.iter_mut() {
                rollback.new_frame_from_world(world);
            }
        }
    }

    // All rollbacking has been done by this point, now simulate the current frame.
    world.run_schedule(GameLogic);

    // Track new component values in the current frame.
    for rollback in component_rollbacks.0.iter_mut() {
        rollback.new_frame_from_world(world);
    }

    // Add back component rollbacks.
    world.insert_resource(component_rollbacks);
}

fn frame_update(
    mut frame_count: ResMut<SyncFrameCount>,
    mut input_rollback: ResMut<InputRollback>,
) {
    frame_count.increment();
    input_rollback.init_current_frame(frame_count.0);
}

/// Rollback plugin:
/// - Update frame count
/// - Initialize input tracker
/// - Collect inputs and game sync, and record rollback requests and game sync requests
/// - Handle rollback requests and game sync requests with `handle_rollback`
pub struct RollbackPluginClient;

impl Plugin for RollbackPluginClient {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                handle_rollback.in_set(ClientSchedule::Rollback),
                frame_update.in_set(ClientSchedule::FrameUpdate),
            ),
        );
    }
}

pub struct RollbackPluginServer;

impl Plugin for RollbackPluginServer {
    fn build(&self, app: &mut App) {
        use crate::schedule::ServerSchedule;

        app.add_systems(
            FixedUpdate,
            (
                handle_rollback.in_set(ServerSchedule::Rollback),
                frame_update.in_set(ServerSchedule::FrameUpdate),
            ),
        );
    }
}
