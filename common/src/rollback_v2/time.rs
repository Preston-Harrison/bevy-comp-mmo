use bevy::{ecs::system::Resource, time::Time};

#[derive(Resource, Default)]
pub struct GameTime {
	delta_seconds: f32,
}

impl GameTime {
	pub fn set_from_time(&mut self, time: Time) {
		self.delta_seconds = time.delta_seconds()
	}

	pub fn delta_seconds(&self) -> f32 {
		self.delta_seconds
	}
}