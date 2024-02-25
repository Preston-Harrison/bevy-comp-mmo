/// First element rollback deques is the transform in the current frame. This is reset in `Rollback::Init`.
/// Rollbacks are only valid after all local and remote input collection and game syncs.
use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, hash::Hash};

use crate::{
    game::GameLogic,
    schedule::{ClientSchedule, ClientState},
    GameSync, IdPlayerInput, Player, PlayerId, RawPlayerInput, ServerEntityMap,
};

pub mod time;

#[derive(Resource, Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SyncFrameCount {
    count: u64,
}

impl SyncFrameCount {
    pub fn new(count: u64) -> Self {
        Self { count }
    }

    fn increment(&mut self) {
        self.count += 1;
    }

    pub fn count(&self) -> u64 {
        self.count
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
        if current_frame != self.current_frame + 1 {
            panic!(
                "Tracker for {} tried to initialize frame {} when internal frame state is {}",
                std::any::type_name::<V>(),
                current_frame,
                self.current_frame
            );
        }
        self.current_frame = current_frame;

        self.history.push_front(HashMap::default());
        if self.history.len() > self.rollback_window {
            self.history.pop_back();
        }
    }

    fn get_n_frames_ago(&self, n_frames: u64) -> Option<&HashMap<K, V>> {
        self.history.get(n_frames as usize)
    }

    fn get_at_frame(&self, frame: u64) -> Option<&HashMap<K, V>> {
        assert!(
            self.current_frame >= frame,
            "Cannot get value at frame. frame = {}, current_frame = {}",
            frame,
            self.current_frame
        );
        self.get_n_frames_ago(self.current_frame - frame)
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
            "Cannot set value at frame. frame = {}, current_frame = {}",
            frame,
            self.current_frame
        );
        self.history
            .get_mut((self.current_frame - frame) as usize)
            .and_then(|map| map.insert(key, value));
    }
}

trait ComponentRollback: Sync + Send {
    fn new_frame_from_world(&mut self, world: &mut World, frame: u64);

    fn rollback_and_sync(&mut self, world: &mut World, game_sync: &GameSync);

    fn rollback_and_update_world(&mut self, frames: u64, world: &mut World);

    fn get_current_frame(&self) -> u64;
}

impl<T: Component + Clone + std::fmt::Debug> ComponentRollback for RollbackTracker<Entity, T> {
    fn new_frame_from_world(&mut self, world: &mut World, frame: u64) {
        let mut query = world.query::<(Entity, &T)>();
        self.init_current_frame(frame);

        for (entity, component) in query.iter(world) {
            self.set_value_at_frame(entity, component.clone(), frame);
        }
    }

    fn get_current_frame(&self) -> u64 {
        self.current_frame
    }

    /// Syncs `T` in world to current game sync values, rolls back history to game sync frame
    /// and sets the current history frame to the game sync frame.
    fn rollback_and_sync(&mut self, world: &mut World, game_sync: &GameSync) {
        // @TEMP old code here instead of rollback_and_update_world
        // self.delete_n_frames(1 + self.current_frame - game_sync.frame);
        // self.init_current_frame(game_sync.frame);
        self.rollback_and_update_world(1 + self.current_frame - game_sync.frame, world);

        world.resource_scope(|world: &mut World, mut se_map: Mut<ServerEntityMap>| {
            let Some(component_updates) = game_sync.get::<T>() else {
                return;
            };
            for (server_obj, component) in component_updates.iter() {
                let entity = match se_map.get(&server_obj) {
                    Some(entity) => *entity,
                    None => {
                        let entity = world.spawn(*server_obj).id();
                        se_map.insert(*server_obj, entity).unwrap();
                        entity
                    }
                };
                info!(
                    "Setting component {:?} for server object {:?}",
                    component, server_obj
                );
                world.entity_mut(entity).insert(component.clone());
                self.set_value_at_frame(entity, component.clone(), game_sync.frame);
            }
        });
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
    pub fn from_frame(frame: u64) -> Self {
        Self(vec![
            Box::new(TransformRollback::new(frame, DEFAULT_ROLLBACK_WINDOW)),
            Box::new(PlayerRollback::new(frame, DEFAULT_ROLLBACK_WINDOW)),
        ])
    }
}

#[derive(Resource)]
pub struct InputRollback {
    tracker: RollbackTracker<PlayerId, RawPlayerInput>,
    future_frames: Vec<IdPlayerInput>,
}

impl InputRollback {
    pub fn from_frame(frame: u64) -> Self {
        Self {
            tracker: RollbackTracker::new(frame, DEFAULT_ROLLBACK_WINDOW),
            future_frames: Vec::new(),
        }
    }

    pub fn accept_input(&mut self, input: IdPlayerInput) {
        if input.input.frame > self.tracker.current_frame {
            self.future_frames.push(input);
        } else {
            self.tracker
                .set_value_at_frame(input.player_id, input.input.raw, input.input.frame);
        }
    }

    fn get_at_frame(&self, frame: u64) -> Option<&HashMap<PlayerId, RawPlayerInput>> {
        self.tracker.get_at_frame(frame)
    }

    fn init_current_frame(&mut self, current_frame: u64) {
        self.tracker.init_current_frame(current_frame);
        let mut current_frame = Vec::new();
        let mut future_frames = Vec::new();

        for input in self.future_frames.drain(..) {
            if input.input.frame == self.tracker.current_frame {
                current_frame.push(input);
            } else {
                future_frames.push(input);
            }
        }

        for frame in current_frame {
            self.tracker
                .set_value_at_frame(frame.player_id, frame.input.raw, frame.input.frame);
        }

        self.future_frames = future_frames;
    }

    pub fn get_latest(&self) -> Option<&HashMap<PlayerId, RawPlayerInput>> {
        self.tracker.get_latest()
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct InputFrame(HashMap<PlayerId, RawPlayerInput>);

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
    pub fn new(game_sync: GameSync) -> Self {
        Self(Some(game_sync))
    }
    pub fn request(&mut self, game_sync: GameSync) {
        // @TODO - check if game sync is recent.
        self.0 = Some(game_sync);
    }
}

fn handle_rollback(world: &mut World) {
    // @FIXME need to set current input buffer inside simulation, currently it's using the current
    // frame, not the frame at time of rollback.

    // Game sync resource may not exist here, as it does not exist on the server.
    let game_sync_request = world
        .get_resource_mut::<GameSyncRequest>()
        .and_then(|mut x| x.0.take());
    let rollback_request = world
        .get_resource_mut::<RollbackRequest>()
        .unwrap()
        .0
        .take();
    let frame_count = world.get_resource::<SyncFrameCount>().unwrap().count();

    // Pop transform out of world so it can be edited mutably alongside world.
    let mut component_rollbacks = world.remove_resource::<ComponentRollbacks>().unwrap();

    macro_rules! simulate_frame {
        ($frame:expr) => {
            world.resource_scope(|world, input_rollback: Mut<'_, InputRollback>| {
                let input_frame = InputFrame(
                    input_rollback
                        .get_at_frame($frame)
                        .cloned()
                        .unwrap_or_default(),
                );
                world.insert_resource(input_frame);
                world.run_schedule(GameLogic);
                for rollback in component_rollbacks.0.iter_mut() {
                    rollback.new_frame_from_world(world, $frame);
                }
                world.remove_resource::<InputFrame>();
            });
        };
    }

    'sim: {
        if let Some(game_sync) = game_sync_request {
            info!(
                "Applying game sync on frame {}, current frame is {}",
                game_sync.frame, frame_count
            );
            if game_sync.frame > frame_count {
                info!("Rolling forward to game sync frame");
                let roll_forward_count = game_sync.frame - frame_count;
                world.get_resource_mut::<SyncFrameCount>().unwrap().count = game_sync.frame + 1;
                info!(
                    "Input current frame is {}",
                    world
                        .get_resource::<InputRollback>()
                        .unwrap()
                        .tracker
                        .current_frame
                );
                info!(
                    "Component current frame is {}",
                    component_rollbacks.0[0].get_current_frame()
                );

                for n in 0..=roll_forward_count {
                    world
                        .get_resource_mut::<InputRollback>()
                        .unwrap()
                        .init_current_frame(frame_count + n + 1);
                }

                for n in 0..=roll_forward_count {
                    info!("Simulating step {} in game sync roll forward", n);
                    simulate_frame!(frame_count + n);
                }

                for rollback in component_rollbacks.0.iter_mut() {
                    rollback.rollback_and_sync(world, &game_sync);
                }

                simulate_frame!(game_sync.frame + 1);

                break 'sim;
            }
            let rollback_count = frame_count - game_sync.frame;

            info!(
                "Before rollback and sync, component current frame is {}",
                component_rollbacks.0[0].get_current_frame()
            );

            for rollback in component_rollbacks.0.iter_mut() {
                rollback.rollback_and_sync(world, &game_sync);
            }

            info!(
                "After rollback and sync, component current frame is {}",
                component_rollbacks.0[0].get_current_frame()
            );

            for n in 0..rollback_count {
                info!("Simulating step {} in game sync", n);
                simulate_frame!(game_sync.frame + n + 1);
            }

            info!(
                "After simulation, component current frame is {}",
                component_rollbacks.0[0].get_current_frame()
            );
        } else if let Some(rollback_frame) = rollback_request {
            info!(
                "Applying rollback to frame {}, current frame is {}",
                rollback_frame, frame_count
            );

            if world.get_resource::<InputRollback>().is_some_and(|ir| {
                ir.get_at_frame(rollback_frame)
                    .map_or(false, |f| f.is_empty())
            }) {
                panic!("Rollback frame has no input - why rollback at all?");
            }

            // @TODO don't allow rollbacks that go further back than a game sync.
            let rollback_count = frame_count - rollback_frame;
            for rollback in component_rollbacks.0.iter_mut() {
                rollback.rollback_and_update_world(rollback_count + 1, world);
            }

            for n in 0..=rollback_count {
                info!("Simulating step {} in rollback", n);
                simulate_frame!(frame_count - rollback_count + n);
            }
        } else {
            simulate_frame!(frame_count);
        }
    }

    // Add back component rollbacks.
    world.insert_resource(component_rollbacks);
}

fn frame_update(
    mut frame_count: ResMut<SyncFrameCount>,
    mut input_rollback: ResMut<InputRollback>,
) {
    frame_count.increment();
    input_rollback.init_current_frame(frame_count.count());
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
            )
                .run_if(in_state(ClientState::InGame)),
        );
    }
}

pub struct RollbackPluginServer;

impl Plugin for RollbackPluginServer {
    fn build(&self, app: &mut App) {
        use crate::schedule::ServerSchedule;

        let init_frame = 1u64;
        app.insert_resource(SyncFrameCount::new(init_frame));
        app.insert_resource(ComponentRollbacks::from_frame(init_frame - 1));
        app.insert_resource(RollbackRequest::default());
        app.insert_resource(InputRollback::from_frame(init_frame));

        app.add_systems(
            FixedUpdate,
            (
                handle_rollback.in_set(ServerSchedule::Rollback),
                frame_update.in_set(ServerSchedule::FrameUpdate),
            ),
        );
    }
}
