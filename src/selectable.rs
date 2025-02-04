use bevy::prelude::*;
use super::orbit::*;

pub trait Selectable {
    fn is_selected(&self, camera_ray: Ray3d) -> bool;
}

#[derive(Default, Debug, Clone)]
pub struct SelectableCelestialBody<D> {
    pub transform: Transform,
    pub orbital_plane: InfinitePlane3d,
    pub radius: f32,
    pub data: D
}

impl <D> Selectable for SelectableCelestialBody<D> {

    fn is_selected(&self, camera_ray: Ray3d) -> bool {
        let plane_origin = self.transform.translation;
        let Some(distance) = camera_ray.intersect_plane(plane_origin, self.orbital_plane) else {
            return false;
        };
        let global_cursor = camera_ray.get_point(distance);
        global_cursor.distance(self.transform.translation) < self.radius * 1.5
    }
}

impl <D> Propagatable for SelectableCelestialBody<D> {
    fn position_for(&mut self, orbit: &SatelliteOrbit, scale: f32) {
        let SatellitePose { position, .. } = orbit.to_translation_and_rotation();
        self.transform = Transform::from_translation(position * scale);
    }
}

impl <D> SelectableCelestialBody<D> {

    pub fn initialize_from_orbit(radius: f32, data: D, orbit: &SatelliteOrbit, scale: f32) -> Self {
        let v1 = orbit.get_right_ascention_vector();
        let v2 = orbit.get_encentricity_vector();
        let normal = v1.cross(v2);
        let orbital_plane = InfinitePlane3d::new(normal);
        let radius = radius * scale;

        let mut value = Self {
            transform: Transform::default(),
            orbital_plane,
            radius,
            data,
        };
        value.position_for(orbit, scale);
        value
    }

    pub fn get_mesh(&self) -> Sphere {
        Sphere { radius: self.radius }
    }
}

pub struct ManySelectables<T>(Vec<T>);

impl <T> ManySelectables<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self(values)
    }
}

impl <T: Selectable> ManySelectables<T> {
    pub fn select(&self, camera_ray: Ray3d) -> Option<&T> {
        self.0.iter().find(|s| s.is_selected(camera_ray))
    }
}

impl <C, T: Selectable> ManySelectables<(C, T)> {
    pub fn select_with_context(self, camera_ray: Ray3d) -> Option<(C, T)> {
        self.0.into_iter().find(|(_, t)| t.is_selected(camera_ray)).map(|(c, t)| (c, t))
    }
}