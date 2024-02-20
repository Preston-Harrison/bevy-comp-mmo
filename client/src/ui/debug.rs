use bevy::prelude::*;
use common::{
    rollback::{InputRollback, SyncFrameCount},
    Player, PlayerId, UMFromServer,
};

use crate::{messages::ServerMessageBuffer, LocalPlayer};

use super::UIRoot;

#[derive(Component)]
pub struct InputCounter {
    pub player_id: PlayerId,
    pub count: u64,
}

#[derive(Component)]
pub struct SyncFrameCounter;

pub fn spawn_input_counters(
    mut commands: Commands,
    ui: Query<Entity, With<UIRoot>>,
    player_q: Query<&Player, Added<Player>>,
) {
    let ui_entity = ui.single();
    for player in player_q.iter() {
        commands.entity(ui_entity).with_children(|parent| {
            parent
                .spawn(InputCounter {
                    player_id: player.id,
                    count: 0,
                })
                .insert(TextBundle::from_section(
                    "",
                    TextStyle {
                        font_size: 20.0,
                        ..Default::default()
                    },
                ));
        });
    }
}

pub fn update_input_counters(
    rollback: Res<InputRollback>,
    messages: Res<ServerMessageBuffer>,
    local_player: Res<LocalPlayer>,
    mut text_q: Query<(&mut Text, &mut InputCounter)>,
) {
    for (mut text, mut input_counter) in text_q.iter_mut() {
        let local_input = input_counter.player_id == local_player.id
            && rollback
                .get_latest()
                .is_some_and(|x| x.contains_key(&local_player.id));
        let remote_input = messages.unreliable.iter().any(|msg| {
            if let UMFromServer::IdPlayerInput(input) = msg {
                input.player_id == input_counter.player_id
            } else {
                false
            }
        });
        if local_input || remote_input {
            input_counter.count += 1;
            if local_input {
                text.sections[0].value = format!(
                    "Local player ({}): {}",
                    input_counter.player_id.0, input_counter.count
                );
            } else {
                text.sections[0].value = format!(
                    "Remote player ({}): {}",
                    input_counter.player_id.0, input_counter.count
                );
            }
        }
    }
}

pub fn update_frame_counter(
    frame: Res<SyncFrameCount>,
    mut text_q: Query<(&mut Text, &mut SyncFrameCounter)>,
) {
    for (mut text, _) in text_q.iter_mut() {
        text.sections[0].value = format!("Frame: {}", frame.0);
    }
}
