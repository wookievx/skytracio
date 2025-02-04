use std::fmt::Debug;

use bevy::{log::info, math::{Quat, Vec3}, prelude::Transform};


#[derive(Default, Debug)]
pub struct CameraLock<I>  {
    pub locked_on: I,
    pub lock_transform: Transform,
    pub distance: f32,
    pub is_default: bool,
    pub is_locked: bool
}

#[derive(Default, Clone)]
pub struct StaticLockSettings {
    pub distance_min: f32,
    pub distance_max: f32,
    pub default_orientation: Vec3,
    pub tolerance: f32,
}

impl <I: Debug> CameraLock<I> {

    pub fn lock_on(&mut self, entity: I, transform: Transform, is_default: bool) {
        self.locked_on = entity;
        self.lock_transform = transform;
        self.is_default = is_default;
        self.is_locked = false;
    }

    pub fn zoom_in(&mut self, by_step: f32, min: f32) {
        let distance = self.distance;
        let distance = distance - by_step;
        self.distance = distance.max(min);
    }

    pub fn zoom_out(&mut self, by_step: f32, max: f32) {
        let distance = self.distance;
        let distance = distance + by_step;
        self.distance = distance.min(max);
    }

    pub fn move_towards_lock(&mut self, settings: &StaticLockSettings, location: &mut Transform, dt: f32) {
        const SPEED: f32 = 1.0;
        let target_location = if self.lock_transform.translation.length() < 0.1 || self.is_default {
            settings.default_orientation * self.distance
        } else {
            let lock_translation = self.lock_transform.translation;
            lock_translation + lock_translation.normalize() * self.distance
        };

        if self.is_locked {
            location.translation = target_location;
        } else {
            let transfer_vector = target_location - location.translation;
            let speed = SPEED * settings.distance_max / transfer_vector.length() + 0.1;
            let change = transfer_vector * speed * dt;
            if transfer_vector.length() < settings.tolerance {
                self.is_locked = true;
                info!("Locking onto {:?}", self);
                location.translation = target_location;
            } else {
                location.translation += change;
            }
        }

        
        self.rotate_to_position(target_location, &mut location.rotation, dt);
    }

    //default rotation is looking at the planet through the satelite
    fn rotate_to_position(&mut self, target_location: Vec3, rotation: &mut Quat, dt: f32) {
        let up_vector = if self.is_default { Vec3::X } else { Vec3::Z };
        let target_rotation = Transform::from_translation(target_location).looking_at(Vec3::ZERO, up_vector).rotation;
        if self.is_locked {
            *rotation = target_rotation;
        } else {
            const SPEED: f32 = 2.0;
            *rotation = rotation.lerp(target_rotation, dt * SPEED).normalize();
        }
    }

}