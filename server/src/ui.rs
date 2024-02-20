use bevy::{prelude::*, utils::HashMap};
use common::{rollback::SyncFrameCount, schedule::ClientSchedule, Player, PlayerId};

#[derive(Resource, Default)]
pub struct InputTracker {
    pub inputs: HashMap<PlayerId, u64>,
}

#[derive(Component)]
pub struct UIRoot;

pub fn setup_ui(mut commands: Commands) {
    commands
        .spawn(UIRoot)
        .insert(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(SyncFrameCounter)
                .insert(TextBundle::from_section(
                    "Frame: -",
                    TextStyle {
                        font_size: 20.0,
                        ..Default::default()
                    },
                ));
        });
    commands.spawn(Camera2dBundle::default());
}

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputTracker>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                FixedUpdate,
                (
                    spawn_input_counters,
                    update_input_counters,
                    update_frame_counter,
                )
                    .chain()
                    .in_set(ClientSchedule::UI),
            );
    }
}

#[derive(Component)]
pub struct SyncFrameCounter;

#[derive(Component)]
pub struct InputCounter {
    pub player_id: PlayerId,
}

fn spawn_input_counters(
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

fn update_input_counters(
    input_tracker: Res<InputTracker>,
    mut text_q: Query<(&mut Text, &InputCounter)>,
) {
    for (mut text, input_counter) in text_q.iter_mut() {
        let count = input_tracker
            .inputs
            .get(&input_counter.player_id)
            .copied()
            .unwrap_or(0);
        text.sections[0].value = format!("Player {}: {}", input_counter.player_id.0, count);
    }
}

fn update_frame_counter(
    sync_frame_count: Res<SyncFrameCount>,
    mut text_q: Query<(&mut Text, &SyncFrameCounter)>,
) {
    for (mut text, _) in text_q.iter_mut() {
        text.sections[0].value = format!("Frame: {}", sync_frame_count.0);
    }
}
