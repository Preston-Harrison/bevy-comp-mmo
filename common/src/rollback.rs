use bevy::{prelude::*, utils::HashMap};
use std::collections::VecDeque;

use crate::{InputBuffer, Player, ServerObject};

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

#[derive(Resource)]
pub struct InputRollback {
    history: VecDeque<InputBuffer>,
}

pub fn track_rollbacks(
    transform_q: Query<(Entity, &Transform), With<ServerObject>>,
    input_buffer: Res<InputBuffer>,
    mut input_rollback: ResMut<InputRollback>,
    mut transform_rollback: ResMut<TransformRollback>,
) {
    let track_num = 10;
    for (entity, transform) in transform_q.iter() {
        let history = transform_rollback.history.entry(entity).or_default();
        add_with_cap(history, *transform, track_num);
    }

    add_with_cap(&mut input_rollback.history, input_buffer.clone(), track_num);
}

fn add_with_cap<T>(vec: &mut VecDeque<T>, item: T, cap: usize) {
    vec.push_front(item);
    if vec.len() > cap {
        vec.pop_back();
    }
}

#[derive(Resource)]
pub struct RollbackRequest(Option<u64>);

pub fn rollback(
    mut request: ResMut<RollbackRequest>,
    mut transform_q: Query<(Entity, &mut Transform)>,
    player_q: Query<(Entity, &Player)>,
    mut transform_rollback: ResMut<TransformRollback>,
    input_rollback: Res<InputRollback>,
) {
    let Some(frames) = request.0 else {
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
