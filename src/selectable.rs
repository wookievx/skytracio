use bevy::prelude::*;

pub trait Selectable {
    fn is_selected(&self, camera_ray: Ray3d) -> bool;
}

#[derive(Default, Clone)]
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
        global_cursor.distance(self.transform.translation) < self.radius
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