use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::{impl_inner, is_server, schedule::GameSchedule, IdPlayerInput, InputBuffer, Player, ServerObject};

#[derive(Resource, Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SyncFrameCount(pub u64);
impl_inner!(SyncFrameCount, u64);

pub fn increment_sync_frame_count(mut frame_count: ResMut<SyncFrameCount>) {
    frame_count.0 += 1;
}

pub const ROLLBACK_WINDOW: usize = 10;

#[derive(Resource, Default)]
pub struct TransformRollback {
    // First element of the deque is the most recent transform.
    history: HashMap<Entity, VecDeque<Transform>>,

}

impl TransformRollback {
    fn get_n_frames_ago(&self, n_frames: u64) -> HashMap<Entity, Transform> {
        let mut result = HashMap::default();
        for (entity, history) in self.history.iter() {
            if let Some(item) = history.get(n_frames as usize) {
                result.insert(*entity, *item);
            }
        }
        result
    }

    fn get_latest(&self) -> HashMap<Entity, Transform> {
        self.get_n_frames_ago(0)
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

#[derive(Debug, Resource, Default)]
pub struct InputRollback {
    history: VecDeque<InputBuffer>,
    current_frame: u64,
}

impl InputRollback {
    pub fn accept_input(&mut self, id_input: IdPlayerInput) {
        if id_input.input.frame > self.current_frame {
            warn!(
                "Input frame {} is from the future, current frame {}",
                id_input.input.frame, self.current_frame
            );
            return;
        }
        let ix = self.current_frame - id_input.input.frame;
        let Some(entry) = self.history.get_mut(ix as usize) else {
            warn!("Input frame {} is too old", id_input.input.frame);
            return;
        };
        entry.0.insert(id_input.player_id, id_input.input.raw);
    }

    pub fn next_frame(&mut self, current_frame: u64) {
        push_front_with_cap(&mut self.history, InputBuffer::default(), ROLLBACK_WINDOW);
        self.current_frame = current_frame;
    }

    pub fn get_at_frame(&self, frame: u64) -> &InputBuffer {
        let ix = self.current_frame - frame;
        self.history.get(ix as usize).unwrap()
    }

    pub fn get_latest(&self) -> &InputBuffer {
        self.history.front().unwrap()
    }

    pub fn get_n_frames_ago(&self, n_frames: u64) -> Option<&InputBuffer> {
        self.history.get(n_frames as usize)
    }
}

pub fn track_rollbacks_components(
    transform_q: Query<(Entity, &Transform), With<ServerObject>>,
    mut transform_rollback: ResMut<TransformRollback>,
) {
    for (entity, transform) in transform_q.iter() {
        let history = transform_rollback.history.entry(entity).or_default();
        push_front_with_cap(history, *transform, ROLLBACK_WINDOW);
    }
}

fn push_front_with_cap<T>(vec: &mut VecDeque<T>, item: T, cap: usize) {
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

fn frame_diff(frame: &SyncFrameCount, request: &RollbackRequest) -> Option<u64> {
    let Some(rollback_to_frame) = request.0 else {
        return None;
    };
    if frame.0 < rollback_to_frame {
        warn!(
            "Cannot rollback to frame {} because current frame is {}",
            rollback_to_frame, frame.0
        );
        return None;
    };
    if frame.0 == rollback_to_frame {
        return Some(0);
    }
    Some(frame.0 - rollback_to_frame)
}

pub fn rollback(
    mut request: ResMut<RollbackRequest>,
    mut transform_q: Query<(Entity, &mut Transform)>,
    player_q: Query<(Entity, &Player)>,
    mut transform_rollback: ResMut<TransformRollback>,
    frame: Res<SyncFrameCount>,
    input_rollback: Res<InputRollback>,
) {
    let Some(frames) = frame_diff(&frame, &request) else {
        return;
    };
    if frames == 0 {
        info!("Rollback to the same frame requested, ignoring");
        request.0 = None;
        return;
    }

    resimulate_last_n_frames(
        frames,
        transform_q
            .iter_mut()
            .map(|(e, t)| (e, t.into_inner()))
            .collect::<Vec<_>>()
            .as_mut_slice(),
        player_q.iter().collect::<Vec<_>>().as_slice(),
        &mut transform_rollback,
        &input_rollback,
    );

    request.0 = None;
}

pub fn resimulate_last_n_frames(
    last_n_frames: u64,
    current_transforms: &mut [(Entity, &mut Transform)],
    current_players: &[(Entity, &Player)],
    transform_rollback: &mut TransformRollback,
    input_rollback: &InputRollback,
) {
    info!("Rolling back {} frames", last_n_frames);
    transform_rollback.rollback_frames(last_n_frames);

    let mut resimulated_transforms = transform_rollback.get_latest();
    
    if is_server() {
        info!(?resimulated_transforms);
    }

    if is_server() {
        info!(?input_rollback);
    }

    for frame in (0..=last_n_frames).rev() {
        let Some(input_for_frame) = input_rollback.get_n_frames_ago(frame) else {
            warn!("No input for frame {}", frame);
            break;
        };

        let mut player_transforms = current_players
            .iter()
            .filter_map(|(entity, player)| {
                resimulated_transforms
                    .get(entity)
                    .map(|transform| (entity, (*player, *transform)))
            })
            .collect::<HashMap<_, _>>();

        let mutable_transforms = player_transforms
            .iter_mut()
            .map(|(_entity, (player, transform))| (*player, transform));

        if is_server() {
            info!(frame, ?input_for_frame);
        }

        super::process_input(
            input_for_frame,
            mutable_transforms,
            super::FRAME_DURATION_SECONDS as f32,
        );

        for (entity, (_player, transform)) in player_transforms.iter() {
            resimulated_transforms.insert(**entity, *transform);
        }
    }

    if is_server() {
        info!(?resimulated_transforms);
    }

    for (entity, transform) in current_transforms.iter_mut() {
        if let Some(new_transform) = resimulated_transforms.get(entity) {
            **transform = *new_transform;
        }
    }
}

pub fn next_input_frame(mut input_rollback: ResMut<InputRollback>, frame: Res<SyncFrameCount>) {
    input_rollback.next_frame(frame.0);
}

pub struct RollbackPlugin;

impl Plugin for RollbackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (increment_sync_frame_count, next_input_frame)
                .chain()
                .in_set(GameSchedule::Init),
        )
        .add_systems(
            FixedUpdate,
            (track_rollbacks_components, rollback).in_set(GameSchedule::Rollback),
        )
        .init_resource::<SyncFrameCount>()
        .init_resource::<InputRollback>()
        .init_resource::<RollbackRequest>()
        .init_resource::<TransformRollback>();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::FRAME_DURATION_SECONDS;

    fn new_entity(id: u64) -> Entity {
        Entity::from_bits(id | 0 >> 32)
    }

    fn gen_transforms() -> Vec<(Entity, Transform)> {
        vec![
            (new_entity(0), Transform::default()),
            (new_entity(1), Transform::default()),
            (new_entity(2), Transform::default()),
        ]
    }

    fn gen_players() -> Vec<(Entity, Player)> {
        let speed = 1.0;
        vec![
            (new_entity(0), Player::new(0.into()).with_speed(speed)),
            (new_entity(1), Player::new(1.into()).with_speed(speed)),
            (new_entity(2), Player::new(2.into()).with_speed(speed)),
        ]
    }

    fn sliceify_mut<T>(vec: &mut Vec<(Entity, T)>) -> Vec<(Entity, &mut T)> {
        vec.iter_mut().map(|(e, t)| (*e, t)).collect::<Vec<_>>()
    }

    fn sliceify<T>(vec: &Vec<(Entity, T)>) -> Vec<(Entity, &T)> {
        vec.iter().map(|(e, t)| (*e, t)).collect::<Vec<_>>()
    }

    #[test]
    fn test_resimulate_last_n_frames_simple() {
        // Create test data
        let last_n_frames = 3;

        let mut current_transforms = gen_transforms();
        let current_players = gen_players();

        let mut transform_rollback = TransformRollback::default();
        let mut input_rollback = InputRollback::default();

        for (entity, transform) in current_transforms.iter() {
            transform_rollback.history.insert(
                *entity,
                VecDeque::from(vec![*transform, *transform, *transform]),
            );
        }

        let mut input_buffer = InputBuffer::default();
        input_buffer
            .0
            .insert(0.into(), crate::RawPlayerInput { x: 1, y: 0 });
        input_rollback.history.push_front(input_buffer);

        let mut input_buffer = InputBuffer::default();
        input_buffer
            .0
            .insert(0.into(), crate::RawPlayerInput { x: 0, y: -1 });
        input_rollback.history.push_front(input_buffer);

        // Call the function
        resimulate_last_n_frames(
            last_n_frames,
            sliceify_mut(&mut current_transforms).as_mut_slice(),
            sliceify(&current_players).as_slice(),
            &mut transform_rollback,
            &input_rollback,
        );

        // Assert the results
        assert_eq!(
            current_transforms[0].1,
            Transform::from_translation(Vec3::new(
                FRAME_DURATION_SECONDS as f32,
                -FRAME_DURATION_SECONDS as f32,
                0.0
            ))
        );
        assert_eq!(current_transforms[1].1, Transform::default());
        assert_eq!(current_transforms[2].1, Transform::default());
    }
}
