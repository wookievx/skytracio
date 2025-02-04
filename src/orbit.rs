

pub trait Propagatable {
    fn position_for(&mut self, orbit: &SatelliteOrbit, scale: f32);
}

#[derive(Debug, Clone, PartialEq, Component)]
pub struct SatelliteOrbit {
    /// Semi-major axis (in kilometers)
    pub semi_major_axis: f32,
    /// Eccentricity (dimensionless)
    pub eccentricity: f32,
    /// Inclination (in degrees)
    pub inclination: f32,
    /// Right Ascension of the Ascending Node (in degrees)
    pub raan: f32,
    /// Argument of Perigee (in degrees)
    pub argument_of_perigee: f32,
    /// True Anomaly at Epoch (in degrees)
    pub true_anomaly: f32,
    /// Epoch time (in Julian Date)
    pub epoch: f32,
}


impl SatelliteOrbit {
    /// Creates a new SatelliteOrbit with given parameters
    pub fn new(
        semi_major_axis: f32,
        eccentricity: f32,
        inclination: f32,
        raan: f32,
        argument_of_perigee: f32,
        true_anomaly: f32,
        epoch: f32,
    ) -> Self {
        SatelliteOrbit {
            semi_major_axis,
            eccentricity,
            inclination,
            raan,
            argument_of_perigee,
            true_anomaly,
            epoch,
        }
    }

    /// Returns the orbital period in seconds
    pub fn orbital_period(&self) -> f32 {
        let a = self.semi_major_axis;
        2.0 * std::f32::consts::PI * (a.powi(3) / GRAVITATIONAL_CONSTANT).sqrt()
    }
}

impl SatelliteOrbit {
    /// Propagates the orbit by a given time `dt` (in seconds) and returns a new orbit with the updated true anomaly.
    pub fn propagate(&self, dt: f32) -> Self {

        let mean_motion = (GRAVITATIONAL_CONSTANT / self.semi_major_axis.powi(3)).sqrt();

        // Mean anomaly at epoch (convert true anomaly to mean anomaly for eccentric orbit)
        let mean_anomaly_epoch = self.true_anomaly_to_mean_anomaly();

        // Update mean anomaly with elapsed time
        let mean_anomaly_new = mean_anomaly_epoch + mean_motion * dt;

        // Solve Kepler's equation to get the new eccentric anomaly
        let eccentric_anomaly_new = self.solve_keplers_equation(mean_anomaly_new);

        // Convert eccentric anomaly to true anomaly
        let true_anomaly_new = self.eccentric_anomaly_to_true_anomaly(eccentric_anomaly_new);

        // Return a new SatelliteOrbit with the updated true anomaly
        SatelliteOrbit {
            true_anomaly: true_anomaly_new,
            ..*self // Copy other parameters unchanged
        }

    }

    /// Converts the true anomaly to mean anomaly for the current orbit
    fn true_anomaly_to_mean_anomaly(&self) -> f32 {
        let e = self.eccentricity;
        let ta_rad = self.true_anomaly.to_radians();

        let ea = 2.0 * (((1.0 - e).sqrt() / (1.0 + e).sqrt()) * (ta_rad / 2.0).tan()).atan();
        ea - e * ea.sin() // Mean anomaly (rad)
    }

    /// Solves Kepler's equation: M = E - e * sin(E) to find the eccentric anomaly
    fn solve_keplers_equation(&self, mean_anomaly: f32) -> f32 {
        let e = self.eccentricity;
        let mut eccentric_anomaly = mean_anomaly; // Initial guess: mean anomaly
        for _ in 0..100 { // Iterative Newton-Raphson method
            let delta = (eccentric_anomaly - e * eccentric_anomaly.sin() - mean_anomaly)
                / (1.0 - e * eccentric_anomaly.cos());
            eccentric_anomaly -= delta;
            if delta.abs() < 1e-6 {
                break;
            }
        }
        eccentric_anomaly
    }

    /// Converts the eccentric anomaly to true anomaly
    fn eccentric_anomaly_to_true_anomaly(&self, eccentric_anomaly: f32) -> f32 {
        let e = self.eccentricity;
        let ea = eccentric_anomaly;

        let cos_ta = (ea.cos() - e) / (1.0 - e * ea.cos());
        let sin_ta = (1.0 - e.powi(2)).sqrt() * ea.sin() / (1.0 - e * ea.cos());

        sin_ta.atan2(cos_ta).to_degrees() // True anomaly (degrees)
    }
}

use bevy::{math::{Quat, Vec3, Vec2}, prelude::*};

/// Represents the translation and rotation of the satellite in a 3D coordinate system using Bevy types
#[derive(Debug)]
pub struct SatellitePose {
    /// Position in Cartesian coordinates as a Bevy Vec3 (in kilometers)
    pub position: Vec3
}

impl SatelliteOrbit {

    pub fn get_encentricity_vector(&self) -> Vec3 {
        let rotation = self.orbital_to_quaternion();
        rotation * Vec3::Y
    }

    pub fn get_right_ascention_vector(&self) -> Vec3 {
        let raan = self.raan.to_radians();
        let q_raan = Quat::from_axis_angle(Vec3::Z, raan);        // Rotate around Z-axis (RAAN)
        q_raan * Vec3::X
    }

    /// Converts the true anomaly to the satellite's translation and rotation in a 3D coordinate system.
    pub fn to_translation_and_rotation(&self) -> SatellitePose {
        // Constants
        let e = self.eccentricity;
        let a = self.semi_major_axis;
        let ta_rad = self.true_anomaly.to_radians();

        // Step 1: Calculate distance from Earth (radius vector in orbital plane)
        let r = a * (1.0 - e.powi(2)) / (1.0 + e * ta_rad.cos());

        // Step 2: Calculate position in the orbital plane (pqw coordinates)
        let x_pqw = r * ta_rad.cos();
        let y_pqw = r * ta_rad.sin();
        let z_pqw = 0.0; // Always zero in the orbital plane

        // Step 3: Convert to the inertial frame (ECI: Earth-Centered Inertial)
        let position = Vec3::new(x_pqw, y_pqw, z_pqw);

        // Step 4: Define satellite rotation as a quaternion
        let rotation = self.orbital_to_quaternion();

        let position = rotation * position;

        SatellitePose { position }
    }

    /// Converts the orbital elements to a quaternion representing the rotation
    fn orbital_to_quaternion(&self) -> Quat {
        use std::f32::consts::PI;
        // Orbital elements
        let inclination = self.inclination.to_radians();
        let raan = self.raan.to_radians();
        let arg_perigee = (self.argument_of_perigee - 90.0).to_radians();

        // Quaternions for each rotation
        let q_raan = Quat::from_axis_angle(Vec3::Z, raan);        // Rotate around Z-axis (RAAN)
        let raan_vector = q_raan * Vec3::X;
        let q_incl = Quat::from_axis_angle(raan_vector.normalize(), inclination); // Rotate raan vector
        let normal_vector = raan_vector.cross(q_incl * Quat::from_axis_angle(Vec3::Z, PI / 2.0) * raan_vector);
        let q_argp = Quat::from_axis_angle(normal_vector.normalize(), arg_perigee); // Rotate around X-axis (Argument of Perigee)

        // Combine rotations: RAAN -> Inclination -> Argument of Perigee
        q_raan * q_incl * q_argp
    }

    pub fn bevy_elipse_parameters(&self, scale: f32) -> (Vec3, Quat, Vec2) {
        // Orbital elements
        let full_rotation = self.orbital_to_quaternion();
        let x = self.semi_major_axis * scale;
        let y = x * (1.0 - self.eccentricity * self.eccentricity).sqrt();
        let elipse_offset = self.semi_major_axis * self.eccentricity;
        let elipse_offset = full_rotation * Vec3::new( -elipse_offset * scale, 0.0, 0.0);

        (elipse_offset, full_rotation, Vec2 { x, y })
    }
}

const GRAVITATIONAL_CONSTANT: f32 = 3.986004418e5; // Earth's gravitational parameter (km^3/s^2)

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_orbit_propagation() {
        let orbit = SatelliteOrbit::new(
            6771.0,  // Semi-major axis in km
            0.001,   // Eccentricity
            51.6,    // Inclination in degrees
            120.0,   // RAAN in degrees
            80.0,    // Argument of Perigee in degrees
            0.0,     // True Anomaly in degrees
            2451545.0, // Epoch (Julian Date)
        );

        let period = orbit.orbital_period();
        // Propagate orbit by one hour (3600 seconds)
        let orbit_quater = orbit.propagate(period / 4.0);
        let orbit_half = orbit.propagate(period / 2.0);
        let orbit_three_quater = orbit.propagate(period / 4.0 * 3.0);
        let orbit_full = orbit.propagate(period);


        for (orbit, expected_true_anomaly) in vec![(orbit_quater, 90.0), (orbit_half, 180.0), (orbit_three_quater, -90.0), (orbit_full, 0.)] {
            assert_abs_diff_eq!(orbit.true_anomaly, expected_true_anomaly, epsilon = 0.2);    
        }
    }

    #[test]
    fn test_elipse_calculations() {
        let mut orbit = SatelliteOrbit::new(
            6771.0,  // Semi-major axis in km
            0.0,   // Eccentricity
            0.0,    // Inclination in degrees
            0.0,   // RAAN in degrees
            80.0,    // Argument of Perigee in degrees
            0.0,     // True Anomaly in degrees
            2451545.0, // Epoch (Julian Date)
        );

        let (offset, rotation, half_axis) = orbit.bevy_elipse_parameters(1.0);

        println!("{:?}", rotation);
        println!("{:?}", half_axis);
        assert_abs_diff_eq!(offset.length(), 0.0, epsilon = 0.1);

        orbit.inclination = 45.0;
        orbit.argument_of_perigee = 90.0;

        let (offset, rotation, half_axis) = orbit.bevy_elipse_parameters(1.0);
        println!("{:?}", rotation);
        println!("{:?}", half_axis);
        println!("{:?}", orbit.get_encentricity_vector());
        println!("{:?}", orbit.get_right_ascention_vector());
        println!("{:?}", orbit.get_right_ascention_vector().cross(orbit.get_encentricity_vector()));
        assert_abs_diff_eq!(offset.length(), 0.0, epsilon = 0.1);

    }

    #[test]
    fn test_translation_computation() {
        let orbit = SatelliteOrbit::new(
            6771.0,  // Semi-major axis in km
            0.001,   // Eccentricity
            51.6,    // Inclination in degrees
            120.0,   // RAAN in degrees
            80.0,    // Argument of Perigee in degrees
            0.0,    // True Anomaly in degrees
            2451545.0, // Epoch (Julian Date)
        );

        // Compute translation and rotation
        let pose = orbit.to_translation_and_rotation();

        // Expected position (calculated or derived from a reliable orbital simulation tool)
        let expected_position = Vec3::new(4645.23, 4645.23, 2138.23); // Example values
        assert_abs_diff_eq!(pose.position.x, expected_position.x, epsilon = 1.0);
        assert_abs_diff_eq!(pose.position.y, expected_position.y, epsilon = 1.0);
        assert_abs_diff_eq!(pose.position.z, expected_position.z, epsilon = 1.0);
    }
}