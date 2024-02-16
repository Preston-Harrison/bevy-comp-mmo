use bevy::{prelude::*, utils::HashMap};
use std::collections::VecDeque;

use crate::{schedule::GameSchedule, FrameCount, IdPlayerInput, InputBuffer, Player, ServerObject};

#[derive(Resource, Default)]
pub struct TransformRollback {
    // First element of the deque is the most recent transform.
    history: HashMap<Entity, VecDeque<Transform>>,
}

impl TransformRollback {
    fn get_at_frame(&self, frame: u64) -> HashMap<Entity, Transform> {
        let mut result = HashMap::default();
        for (entity, history) in self.history.iter() {
            if let Some(transform) = history.get(frame as usize) {
                result.insert(*entity, *transform);
            }
        }
        result
    }

    fn rollback_frames(&mut self, frame: u64) {
        for history in self.history.values_mut() {
            for _ in 0..frame {
                if history.len() > 1 {
                    history.pop_front();
                }
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct InputRollback {
    history: VecDeque<InputBuffer>,
    current_frame: u64,
}

impl InputRollback {
    pub fn accept_input(&mut self, id_input: IdPlayerInput) {
        let ix = self.current_frame - id_input.input.frame;
        let Some(entry) = self.history.get_mut(ix as usize) else {
            warn!("Input frame {} is too old", id_input.input.frame);
            return;
        };
        entry.0.insert(id_input.player_id, id_input.input.raw);
    }

    pub fn next_frame(&mut self, current_frame: u64) {
        self.history.push_front(InputBuffer::default());
        self.current_frame = current_frame;
    }

    pub fn get_at_frame(&self, frame: u64) -> &InputBuffer {
        let ix = self.current_frame - frame;
        self.history.get(ix as usize).unwrap()
    }

    pub fn get_latest(&self) -> &InputBuffer {
        self.history.front().unwrap()
    }
}

pub fn track_rollbacks_components(
    transform_q: Query<(Entity, &Transform), With<ServerObject>>,
    mut transform_rollback: ResMut<TransformRollback>,
) {
    let track_num = 10;
    for (entity, transform) in transform_q.iter() {
        let history = transform_rollback.history.entry(entity).or_default();
        add_with_cap(history, *transform, track_num);
    }
}

fn add_with_cap<T>(vec: &mut VecDeque<T>, item: T, cap: usize) {
    vec.push_front(item);
    if vec.len() > cap {
        vec.pop_back();
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

pub fn rollback(
    mut request: ResMut<RollbackRequest>,
    mut transform_q: Query<(Entity, &mut Transform)>,
    player_q: Query<(Entity, &Player)>,
    mut transform_rollback: ResMut<TransformRollback>,
    frame: Res<FrameCount>,
    input_rollback: Res<InputRollback>,
) {
    let Some(frames) = request
        .0
        .map(|rollback_to_frame| frame.0 - rollback_to_frame)
    else {
        request.0 = None;
        return;
    };

    info!("Rolling back {} frames", frames);
    transform_rollback.rollback_frames(frames);

    let mut current_transforms = transform_rollback.get_at_frame(frames);

    for frame in 0..frames {
        let Some(input_for_frame) = input_rollback.history.get(frame as usize) else {
            break;
        };

        let mut player_transforms = player_q
            .iter()
            .filter_map(|(entity, player)| {
                current_transforms
                    .get(&entity)
                    .map(|transform| (entity, (player, *transform)))
            })
            .collect::<HashMap<_, _>>();

        let mutable_transforms = player_transforms
            .iter_mut()
            .map(|(_entity, (player, transform))| (*player, transform));

        super::process_input(
            input_for_frame,
            mutable_transforms,
            super::FRAME_DURATION_SECONDS as f32,
        );

        for (entity, (_player, transform)) in player_transforms.iter() {
            current_transforms.insert(*entity, *transform);
        }
    }

    for (entity, mut transform) in transform_q.iter_mut() {
        if let Some(new_transform) = current_transforms.get(&entity) {
            *transform = *new_transform;
        }
    }
    request.0 = None;
}

pub fn next_input_frame(mut input_rollback: ResMut<InputRollback>, frame: Res<FrameCount>) {
    input_rollback.next_frame(frame.0);
}

pub struct RollbackPlugin;

pub const ROLLBACK_SYSTEM: &str = "rollback_system";

impl Plugin for RollbackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, next_input_frame.in_set(GameSchedule::Init))
            .add_systems(
                FixedUpdate,
                (track_rollbacks_components, rollback).in_set(GameSchedule::Rollback),
            )
            .init_resource::<InputRollback>()
            .init_resource::<RollbackRequest>()
            .init_resource::<TransformRollback>();
    }
}
