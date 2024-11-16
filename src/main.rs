
mod selectable;

use bevy::prelude::*;
use selectable::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<Game>()
        .init_state::<GameState>()
        .add_systems(Startup, setup_cameras)
        .add_systems(OnEnter(GameState::Playing), setup)
        .add_systems(Update, change_focus.run_if(in_state(GameState::Playing)))
        .add_systems(Update, 
            (propagate_orbit, move_camera)
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
    minimal_distance: f32,
    maximal_distance: f32,
    camera_lock: Transform,
    current_distance: f32
}

#[derive(Default)]
struct Orbit {
    entity: Option<Entity>,
    radius: f32,
    color: Color
}

#[derive(Default)]
struct Planet {
    entity: Option<Entity>,
    celestial: SelectableCelestialBody<u8>,
    color: Color
}

#[derive(Default)]
struct Moon {
    entity: Option<Entity>,
    celestial: SelectableCelestialBody<u8>,
    color: Color,
}

#[derive(Resource, Default)]
struct Game {
    planet: Planet,
    orbit: Orbit,
    moon: Moon,
    settings: GlobalSettings,
    camera_transform: Transform,
    current_focus: u8
}

fn setup_cameras(mut commands: Commands, mut game: ResMut<Game>) {
    game.settings.camera_lock = Transform::from_translation(Vec3::ZERO);
    game.settings.maximal_distance = 70.0;
    game.settings.minimal_distance = 10.0;
    game.settings.current_distance = 50.0;
    game.camera_transform = Transform::from_xyz(
        0.0,
        game.settings.current_distance,
        0.0,
    )
    .looking_at(Vec3::ZERO, Vec3::Z);
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

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut game: ResMut<Game>) {

    let plane = InfinitePlane3d::new(Vec3::Y);
    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(4.0, 30.0, 4.0),
        point_light: PointLight {
            intensity: 5_000_000.0,
            shadows_enabled: true,
            range: 50.0,
            ..default()
        },
        ..default()
    });

    game.planet.color = Color::linear_rgb(0.0, 0.0, 1.0);
    game.moon.color = Color::linear_rgb(0.5, 0.5, 0.5);
    game.orbit.color = Color::linear_rgb(0.5, 0.5, 0.0);
    game.planet.celestial.radius = 2.0;
    game.planet.celestial.transform = Transform::from_translation(Vec3::ZERO);
    game.planet.celestial.orbital_plane = plane;
    game.planet.celestial.data = 0;
    game.orbit.radius = 20.0;
    game.moon.celestial.radius = 0.2;
    game.moon.celestial.transform = Transform::from_translation(Vec3::NEG_X * game.orbit.radius);
    game.moon.celestial.orbital_plane = plane;
    game.moon.celestial.data = 1;
    game.current_focus = 0;

    let planet_shape = meshes.add(Sphere::default().mesh());
    let moon_shape = meshes.add(Sphere::default().mesh());
    let mut orbit = Torus::default();
    orbit.minor_radius = 0.06;
    orbit.major_radius = game.orbit.radius;
    let orbit = meshes.add(orbit);
    game.orbit.entity = Some(commands.spawn(
        PbrBundle {
            mesh: orbit,
            transform: Transform::from_translation(Vec3::ZERO),
            material: materials.add(game.orbit.color),
            ..default()
        }
    ).id());
    game.moon.entity = Some(commands.spawn(
        PbrBundle {
            mesh: moon_shape,
            transform: game.moon.celestial.transform,
            material: materials.add(game.moon.color),
            ..default()
        }
    ).id());
    game.planet.entity = Some(commands.spawn(
        PbrBundle {
            mesh: planet_shape,
            transform: Transform::from_scale(Vec3::ONE * game.planet.celestial.radius),
            material: materials.add(game.planet.color),
            ..default()
        }
    ).id());
    // spawn the game board
}

// remove all entities that are not a camera or window
fn teardown(mut commands: Commands, entities: Query<Entity, (Without<Camera>, Without<Window>)>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn propagate_orbit(
    time: Res<Time>,
    mut game: ResMut<Game>,
    mut transforms: Query<&mut Transform>
) {
    const SPEED: f32 = 0.5;
    let normal = game.moon.celestial.orbital_plane.normal.as_vec3();
    let rotation = Quat::from_axis_angle(normal, SPEED * time.delta_seconds());
    let planet_location = game.planet.celestial.transform.translation;
    game.moon.celestial.transform.rotate_around(planet_location, rotation);
    let Some(moon) = game.moon.entity else {
        return;
    };

    if game.current_focus == game.moon.celestial.data {
        game.settings.camera_lock = game.moon.celestial.transform;
    }

    if let Ok(mut mesh_transform) = transforms.get_mut(moon) {
        *mesh_transform = game.moon.celestial.transform;
    }
}

fn change_focus(
    q_window: Query<&Window>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
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

    let selectables = ManySelectables::new(vec![game.planet.celestial.clone(), game.moon.celestial.clone()]);

    let Some(selected) = selectables.select(ray) else {
        return;
    };

    game.settings.camera_lock = selected.transform;
    game.current_focus = selected.data;
}

fn move_camera(
    time: Res<Time>,
    mut game: ResMut<Game>,
    mut my_camera: Query<&mut Transform, With<Camera>>
) {    
    if time.delta_seconds() == 0.0 {
        return;
    }
    const SPEED: f32 = 6.0;
    let speed = SPEED * game.settings.maximal_distance / game.settings.current_distance;
    let target_position = game.settings.camera_lock.translation + Vec3::Y * game.settings.current_distance;
    let current_offset = target_position - game.camera_transform.translation;
    let change = current_offset.normalize() * speed * time.delta_seconds();
    let new_position = game.camera_transform.translation + change;
    if current_offset.length() <= 0.1 {
        game.camera_transform.translation = target_position;
    } else {
        game.camera_transform.translation = new_position;
    }
    for mut camera in my_camera.iter_mut() {
        info!("Moved from: {:?} to {:?}", camera, game.camera_transform);
        *camera = game.camera_transform;
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
    if keyboard_input.pressed(KeyCode::KeyI) {
        let c = game.settings.current_distance;
        game.settings.current_distance = game.settings.minimal_distance.max(c - 5.0);
    } else if keyboard_input.pressed(KeyCode::KeyO) {
        let c = game.settings.current_distance;
        game.settings.current_distance = game.settings.maximal_distance.min(c + 5.0);
    }
}
