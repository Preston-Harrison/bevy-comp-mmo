use bevy::prelude::*;
use common::schedule::GameSchedule;

use self::debug::SyncFrameCounter;

mod debug;

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
}

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui).add_systems(
            FixedUpdate,
            (
                debug::spawn_input_counters,
                debug::update_input_counters,
                debug::update_frame_counter,
            )
                .chain()
                .after(GameSchedule::Main)
                .before(GameSchedule::Rollback),
        );
    }
}
