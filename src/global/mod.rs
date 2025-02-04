use std::time::Duration;

use bevy::prelude::Resource;


#[derive(Resource)]
pub struct InGameSettings {
    pub scale: f32,
    pub simulation_speed: f32,
    pub propagation: PropagationSettings
}

pub struct PropagationSettings {
    pub real_time_interval: Duration,
    pub batch_size: usize
}