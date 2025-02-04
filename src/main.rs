
mod selectable;
mod orbit;
mod camera;
mod earth;
mod propagation;
pub mod global;

use std::time::Duration;

use bevy::{color::palettes::css::*, prelude::*};
use camera::{CameraLock, StaticLockSettings};
use earth::{AssetPrepared, LoadAndScaleEarthModelPlugin};
use global::{InGameSettings, PropagationSettings};
use orbit::{Propagatable, SatelliteOrbit};
use selectable::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    Loading,
    Playing,
    GameOver,
}

fn main() {
    App::new()
        .insert_resource(InGameSettings { scale: 0.01, simulation_speed: 1000.0, propagation: PropagationSettings { real_time_interval: Duration::from_secs(2), batch_size: 50 } })
        .insert_resource(propagation::ConstFileClient::new("assets/".into()))
        .add_plugins(DefaultPlugins)
        .add_plugins(LoadAndScaleEarthModelPlugin::<Earth>::new(127.56))
        .add_plugins(propagation::LoadElementsPlugin::<propagation::ConstFileClient>::new())
        .add_plugins(propagation::PropagateElementsPlugin)
        .add_plugins(propagation::PropagateInGamePlugin)
        .init_resource::<Game>()
        .init_state::<GameState>()
        .add_systems(Startup, (setup_cameras, load_data))
        .add_systems(Update, transition_to_playing.run_if(in_state(GameState::Loading)))
        .add_systems(OnEnter(GameState::Playing), setup)
        .add_systems(Update, change_focus.run_if(in_state(GameState::Playing)))
        .add_systems(Update, 
            (propagete_actual_orbit, move_camera, draw_orbits)
                .run_if(in_state(GameState::Playing)))
        .add_systems(
            Update,
            (gameover_keyboard, scroll_update).run_if(in_state(GameState::Playing)),
        )
        .add_systems(OnExit(GameState::GameOver), teardown)
        .run();
}

#[derive(Default)]
struct GlobalSettings {
    lock_settings: StaticLockSettings
}

#[derive(Default)]
struct Planet {
    entity: Option<Entity>,
    celestial: SelectableCelestialBody<u8>,
    color: Color
}

#[derive(Default, Debug, Component)]
struct Satelite {
    celestial: SelectableCelestialBody<u8>,
    color: Color,
}

#[derive(Resource, Default)]
struct Game {
    planet: Planet,
    settings: GlobalSettings,
    camera_transform: Transform,
    camera_lock: CameraLock<u8>
}

#[derive(Component, Default)]
struct Earth;

fn load_data(mut load_elements: EventWriter<propagation::LoadElements>) {
    load_elements.send(propagation::LoadElements { group: "galileo".to_owned(), format: "JSON".to_owned() });
}

fn setup_cameras(mut commands: Commands, mut game: ResMut<Game>) {
    game.settings.lock_settings = StaticLockSettings {
        distance_min: 100.0,
        distance_max: 700.0,
        default_orientation: Vec3::Z,
        tolerance: 1.0
    };
    game.camera_transform = Transform::from_xyz(
        0.0,
          0.0,
        500.0,
    )
    .looking_at(Vec3::ZERO, Vec3::X);
    let camera = Camera3dBundle {
        transform: game.camera_transform,
        projection: PerspectiveProjection {
            // We must specify the FOV in radians.
            // Rust can convert degrees to radians for us.
            fov: 60.0_f32.to_radians(),
            ..default()
        }.into(),
        ..default()
    };

    commands.spawn(camera);
}

fn transition_to_playing(
    mut next_state: ResMut<NextState<GameState>>,
    mut ev_levelup: EventReader<AssetPrepared>,
    mut game: ResMut<Game>
) {
    for ev in ev_levelup.read() {
        game.planet.entity = Some(ev.entity_id.clone());
        next_state.set(GameState::Playing);
    }
}

fn setup(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>, 
    mut game: ResMut<Game>,
    settings: Res<InGameSettings>
) {

    let plane = InfinitePlane3d::new(Vec3::Y);
    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(4.0, 90.0, 4.0),
        point_light: PointLight {
            intensity: 15_000_000.0,
            shadows_enabled: true,
            range: 500.0,
            ..default()
        },
        ..default()
    });

    let moon_orbit = SatelliteOrbit {
        semi_major_axis: 20000.0,
        eccentricity: 0.001,
        inclination: 5.0,
        raan: 0.0,
        argument_of_perigee: 20.0,
        true_anomaly: 0.0,
        epoch: 0.0,
    };
    let moon = Satelite {
        celestial: SelectableCelestialBody::initialize_from_orbit(1000.0, 1, &moon_orbit, settings.scale),
        color: WHITE_SMOKE.into(),
    };

    let moon_2_orbit = SatelliteOrbit {
        semi_major_axis: 24000.0,
        eccentricity: 0.15,
        inclination: 12.0,
        raan: 0.0,
        argument_of_perigee: 90.0,
        true_anomaly: 0.0,
        epoch: 0.0
    };

    let moon_2 = Satelite {
        celestial: SelectableCelestialBody::initialize_from_orbit(1500.0, 2, &moon_2_orbit, settings.scale),
        color: GREEN_YELLOW.into(),
    };

    game.planet.color = Color::linear_rgb(0.0, 0.0, 1.0);
    game.planet.celestial.radius = 6600.0 * settings.scale;
    game.planet.celestial.transform = Transform::from_translation(Vec3::ZERO);
    game.planet.celestial.orbital_plane = plane;
    game.planet.celestial.data = 0;

    let default_transform = Transform::from_xyz(
        0.0,
          0.0,
        500.0,
    )
    .looking_at(Vec3::ZERO, Vec3::Y);
    
    game.camera_lock = CameraLock {
        locked_on: 0, //planet
        lock_transform: Transform::default(),
        distance: default_transform.translation.length(),
        is_default: true,
        is_locked: true
    };

    let moon_shape = meshes.add(moon.celestial.get_mesh().mesh());
    let moon_2_shape = meshes.add(moon_2.celestial.get_mesh().mesh());

    let _ = commands.spawn(
        (PbrBundle {
            mesh: moon_shape,
            transform: moon.celestial.transform,
            material: materials.add(moon.color),
            ..default()
        }, 
        moon_orbit, 
        moon)
    ).id();
    let _ = commands.spawn(
        (PbrBundle {
            mesh: moon_2_shape,
            transform: moon_2.celestial.transform,
            material: materials.add(moon_2.color),
            ..default()
        }, 
        moon_2_orbit,
        moon_2)
    );
}

// remove all entities that are not a camera or window
fn teardown(mut commands: Commands, entities: Query<Entity, (Without<Camera>, Without<Window>)>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn propagete_actual_orbit(
    time: Res<Time>,
    settings: Res<InGameSettings>,
    mut game: ResMut<Game>,
    mut satelites: Query<(&mut Transform, &mut SatelliteOrbit, &mut Satelite)>
) {
    let dt = time.delta_seconds() * settings.simulation_speed;
    for (mut transform, mut orbit, mut satelite) in satelites.iter_mut() {
        let data = satelite.celestial.data;
        *orbit = orbit.propagate(dt);
        satelite.celestial.position_for(&*orbit, settings.scale);
        *transform = satelite.celestial.transform;
        // info!("Propagating orbit: {:?}, {:?} by {:?}", &orbit, &satelite.celestial, dt);
        if game.camera_lock.locked_on == data {
            game.camera_lock.lock_transform = transform.clone();
        }
    }
}

fn change_focus(
    q_window: Query<&Window>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
    q_satelites: Query<(&Transform, &Satelite)>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut game: ResMut<Game>
) {

    if !buttons.pressed(MouseButton::Left) {        
        return;
    }
    let (camera, camera_transform) = q_camera.single();
    let window = q_window.single();

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    let selectables = q_satelites.iter().map(|(t, s)| (t.clone(), s.celestial.clone())).chain(vec![(Transform::from_translation(Vec3::ZERO), game.planet.celestial.clone())]).collect();

    let selectables = ManySelectables::new(selectables);

    let Some((selected_transform, selected)) = selectables.select_with_context(ray) else {
        return;
    };

    game.camera_lock.lock_on(selected.data, selected_transform, selected.data == 0);
}

fn draw_orbits(
    mut gizmos: Gizmos,
    orbits: Query<(&Transform, &SatelliteOrbit)>,
    settings: Res<InGameSettings>
) {
    gizmos.arrow(Vec3::ZERO, Vec3::Z * 70.0, DARK_GRAY);
    gizmos.arrow(Vec3::ZERO, Vec3::Y * 70.0, DARK_GRAY);
    gizmos.arrow(Vec3::ZERO, Vec3::X * 70.0, WHEAT);
    for (pos, orbit) in orbits.iter() {
        let (position, rotation, half_size) = orbit.bevy_elipse_parameters(settings.scale);
        
        // let true_anomaly_adjusted = orbit.true_anomaly as i32;
        // if (true_anomaly_adjusted % 360).abs() < 10 {
        //     gizmos.arrow(Vec3::ZERO, pos.translation, Color::WHITE);
        // } else {
        //     gizmos.arrow(Vec3::ZERO, pos.translation, Color::BLACK);
        // }

        gizmos.ellipse(position, rotation, half_size, Color::linear_rgb(1.0, 0.0, 0.0))
            .resolution(64);
    }
}

fn move_camera(
    time: Res<Time>,
    mut game: ResMut<Game>,
    mut my_camera: Query<&mut Transform, With<Camera>>,
) {    
    if time.delta_seconds() == 0.0 {
        return;
    }
    for mut camera in my_camera.iter_mut() {
        let settings = game.settings.lock_settings.clone();
        game.camera_lock.move_towards_lock(&settings, &mut *camera, time.delta_seconds());
        game.camera_transform = camera.clone();
    }
}

// restart the game when pressing spacebar
fn gameover_keyboard(
    mut next_state: ResMut<NextState<GameState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Playing);
    }
}

fn scroll_update(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut game: ResMut<Game>
) {
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        let min = game.settings.lock_settings.distance_min;
        game.camera_lock.zoom_in(50.0, min);
    } else if keyboard_input.just_pressed(KeyCode::KeyO) {
        let max = game.settings.lock_settings.distance_max;
        game.camera_lock.zoom_out(50.0, max);
    }
}
