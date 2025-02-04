use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use sgp4::{Elements, ElementsError, MinutesSinceEpoch, Prediction};
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Mul};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::orbit::SatelliteOrbit;
use crate::global::*;

use super::{EpochDataLoader, OrbitalData};

pub struct LoadElementsPlugin<C>(PhantomData<C>);

impl <C> LoadElementsPlugin<C> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[derive(Event, Default)]
pub struct LoadElements {
    pub group: String,
    pub format: String
}

#[derive(Event, Default)]
pub struct LoadedElements {
    pub entities: Vec<Entity>,
    pub data: OrbitalData
}

#[derive(Component)]
struct JobInExecution {
    task: Task<OrbitalData>
}

#[derive(Resource)]
struct SateliteDisplayData {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>
}

impl <C: EpochDataLoader + Resource + Clone> Plugin for LoadElementsPlugin<C> {
    fn build(&self, app: &mut App) {
        let rendering_condition = resource_exists::<Assets<Mesh>>.and_then(resource_exists::<Assets<StandardMaterial>>);
        app
          .add_event::<LoadElements>()
          .add_event::<LoadedElements>()
          .add_systems(Startup, create_assets.run_if(rendering_condition.clone()))
          .add_systems(PreUpdate, instantiate_satelite.run_if(rendering_condition))
          .add_systems(Update, move_to_loading::<C>)
          .add_systems(PostUpdate, execute_elements_loading);
    }
}

fn create_assets(mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut commands: Commands) {
    let sphere = Sphere { radius: 1.5 };
    let mesh = meshes.add(sphere.mesh());
    let material = materials.add(Color::WHITE);
    commands.insert_resource(SateliteDisplayData { mesh, material });
}

fn move_to_loading<C: EpochDataLoader + Resource + Clone>(mut load_events: EventReader<LoadElements>, epoch_data_loader: Res<C>, mut commands: Commands) {
    for ev in load_events.read() {
        debug!("Spawning");
        let thread_pool = AsyncComputeTaskPool::get();
        let local_loader = epoch_data_loader.clone();
        let group = ev.group.clone();
        let format = ev.format.clone();

        let task = thread_pool.spawn(async move {
            local_loader.load_or_empty(group, format).await
        });
        commands.spawn_empty()
            .insert(JobInExecution { task });
    }
}

fn execute_elements_loading(
    mut loading_resources: Query<(Entity, &mut JobInExecution)>, mut loaded_data: EventWriter<LoadedElements>, 
    mut commands: Commands
) {
    for (entity, mut job) in loading_resources.iter_mut() {
        debug!("Polling on: {entity}");
        if let Some(data) = block_on(future::poll_once(&mut job.task)) {
            let entities = data.iter().map(|el| {
                let sattelite = PropagatableSattelite::new(InGameElements(el.clone()));
                debug!("Spawning: {:?}", sattelite.orbit);
                commands.spawn(sattelite).id()
            }).collect();
            loaded_data.send(LoadedElements { entities, data });
            commands.get_entity(entity).unwrap().despawn();
        }
    }
}

fn instantiate_satelite(mut loaded_data: EventReader<LoadedElements>, mut commands: Commands, display_data: Res<SateliteDisplayData>) {
    for ev in loaded_data.read() {
        for entity in &ev.entities {
            commands
                .entity(*entity)
                .insert(PbrBundle {
                    mesh: display_data.mesh.clone(),
                    material: display_data.material.clone(),
                    ..default()
                });
        }
    }
}

//propagation plugin
pub struct PropagateElementsPlugin;

#[derive(Clone, Component)]
pub struct InGameElements(pub Arc<Elements>);

#[derive(Component)]
enum PropagationStatus {
    Propagated {
        velocity: Velocity,
        //not a translation of sattelite in-game, but a position as reported by propagator
        position: Vec3,
        just_propagated: bool
    },
    NotPropagated
}

#[derive(Component)]
struct Velocity(Vec3);

impl From<[f64; 3]> for Velocity {
    fn from(value: [f64; 3]) -> Self {
        let [x, y, z] = value;
        Self(Vec3 { x: x as f32, y: y as f32, z: z as f32 })
    }
}

impl Mul<f32> for Velocity {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

#[derive(Bundle)]
pub struct PropagatableSattelite {
    pub elements: InGameElements,
    pub orbit: SatelliteOrbit,
    status: PropagationStatus,
    dt_acc: PropagatableDuration
}

#[derive(Component)]
struct PropagatableDuration(Duration);

impl PropagatableSattelite {
    fn new(elements: InGameElements) -> Self {
        let orbit = elements.0.as_ref().into();
        Self { elements, orbit, status: PropagationStatus::NotPropagated, dt_acc: PropagatableDuration(Duration::ZERO) }
    }
}

impl Add<Duration> for PropagatableDuration {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<Duration> for PropagatableDuration {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs;
    }
}

#[derive(Event)]
pub struct Propagate {
    pub data: Vec<(Entity, InGameElements)>,
    pub dt_minutes: f64
}

#[derive(Debug, Event, Clone)]
pub struct Propageted {
    data: Vec<(Entity, Prediction)>
}

#[derive(Resource, Default)]
struct PropagationResults(Arc<Mutex<Vec<Propageted>>>);

#[derive(Resource)]
struct PropagationTimer {
    timer: Timer
}

impl Plugin for PropagateElementsPlugin {
    fn build(&self, app: &mut App) {

        app
            .insert_resource(PropagationResults::default())
            .add_event::<Propagate>()
            .add_event::<Propageted>()
            .add_systems(Startup, setup_propagation_timer)
            .add_systems(PreUpdate, post_loadup_predictions)
            .add_systems(Update, (accept_propagation, send_predictions))
            .add_systems(PostUpdate, trigger_propagation);
    }
}

fn setup_propagation_timer(settings: Res<InGameSettings>, mut commands: Commands) {
    commands.insert_resource(PropagationTimer { timer: Timer::from_seconds(settings.propagation.real_time_interval.as_secs_f32(), TimerMode::Repeating) });
}

fn trigger_propagation(mut propagate_events: EventWriter<Propagate>, mut timer: ResMut<PropagationTimer>, time: Res<Time>, mut elements: Query<(Entity, &InGameElements, &mut PropagatableDuration)>, settings: Res<InGameSettings>) {

    timer.timer.tick(time.delta());

    if timer.timer.finished() {
        let dt_minutes = settings.propagation.real_time_interval.as_secs_f64() * (settings.simulation_speed as f64) / 60.0;
        let mut data = elements.iter_mut().peekable();

        while let Some((_, _, duration_acc)) = data.peek_mut() {
            *duration_acc.as_mut() += Duration::from_secs_f64(dt_minutes * 60.0);
            let dt_minutes = duration_acc.0.as_secs_f64() / 60.0;
            let data = data.by_ref().take(settings.propagation.batch_size).map(|(entity, d, _)| (entity, d.clone())).collect();
            propagate_events.send(Propagate { data, dt_minutes });
        }
    }

}

fn accept_propagation(mut propagate_events: EventReader<Propagate>, propagations: Res<PropagationResults>) {
    let thread_pool = AsyncComputeTaskPool::get();
    for ev in propagate_events.read() {
        let elements = ev.data.clone();
        let dt = ev.dt_minutes;
        let propagations = Res::clone(&propagations);
        thread_pool.scope(|s| {
            s.spawn(async move {
                do_propagate(propagations, elements, dt);
            });
        });
    }

}

fn do_propagate(propagations: Res<PropagationResults>, elements: Vec<(Entity, InGameElements)>, dt: f64) {
    let data: Result<Vec<(Entity, Prediction)>, PropagationError> = elements.iter().map(|(entity, el)| {
        let constants = sgp4::Constants::from_elements(&el.0)?;
        let prediction = constants.propagate(MinutesSinceEpoch(dt))?;
        Ok((entity.clone(), prediction))
    }).collect();

    match data {
        Ok(data) => {
            let mut lock = propagations.0.lock().unwrap();
            lock.push(Propageted { data });
        },
        Err(err) => {
            error!("Failed to execute propagation: {:?}", err);
        },
    }
}

fn send_predictions(mut propagated_predictions: EventWriter<Propageted>, propagations: Res<PropagationResults>) {
    let mut lock = propagations.0.lock().unwrap();
    for propagated in lock.drain(0..) {
        propagated_predictions.send(propagated);
    }
}

//blocking, limited in scope
fn post_loadup_predictions(mut loaded: EventReader<LoadedElements>, elements: Query<&InGameElements>, propagations: Res<PropagationResults>) {
    //initial propagation is a hack
    for ev in loaded.read() {
        let data = ev.entities.iter().filter_map(|e| elements.get(*e).ok().map(|el| (*e, el.clone()))).collect();
        do_propagate(Res::clone(&propagations), data, 0.01);
    }
}

#[derive(Debug)]
enum PropagationError {
    Elements(ElementsError),
    Propagation(sgp4::Error)
}

impl From<ElementsError> for PropagationError {
    fn from(value: ElementsError) -> Self {
        Self::Elements(value)
    }
}

impl From<sgp4::Error> for PropagationError {
    fn from(value: sgp4::Error) -> Self {
        Self::Propagation(value)
    }
}

//in-game propagation plugin
pub struct PropagateInGamePlugin;


impl Plugin for PropagateInGamePlugin {
    fn build(&self, app: &mut App) {

        app
           .add_systems(Update, adjust_transaltions_on_propagation)
           .add_systems(Update, approximate_propagation);
    }
}

fn adjust_transaltions_on_propagation(mut positions: Query<(&mut Transform, &mut PropagationStatus, &SatelliteOrbit), With<InGameElements>>, mut events: EventReader<Propageted>, settings: Res<InGameSettings>) {
    for propagated in events.read() {
        for (entity, prediction) in &propagated.data {
            let Ok((mut transform, mut status, orbit)) = positions.get_mut(entity.clone()) else {
                continue;
            };

            let [x, y, z] = prediction.position;
            let translation = Vec3 {
                x: x as f32,
                y: y as f32,
                z: z as f32,
            };
            debug!("Got prediction: {:?}, orbit: {:?}", prediction.position, orbit);
            debug!("Distance: {}, orbit semi-major: {:?}", translation.length(), orbit.semi_major_axis);

            transform.translation = translation * settings.scale;
            debug!("In game translaction: {}, elipse params: {:?}", transform.translation.length(), orbit.bevy_elipse_parameters(settings.scale));
            *status = PropagationStatus::Propagated {
                velocity: prediction.velocity.into(),
                position: translation,
                just_propagated: true,
            }
        }
    }
}

fn approximate_propagation(mut satelites: Query<(&mut Transform, &mut PropagationStatus), With<InGameElements>>, time: Res<Time>, settings: Res<InGameSettings>) {
    for (mut t, mut status) in satelites.iter_mut() {

        let velocity = match status.as_mut() {
            PropagationStatus::Propagated { velocity, position, just_propagated } => {
                if *just_propagated {
                    *just_propagated = false;
                    continue;
                }
               &* velocity
            },
            PropagationStatus::NotPropagated => {
                continue; 
            },
        };

        let delta_position = velocity.0 * (settings.scale * settings.simulation_speed * time.delta_seconds());
        t.translation += delta_position;
    }
}

impl From<&sgp4::Elements> for SatelliteOrbit {
    fn from(value: &sgp4::Elements) -> Self {
        SatelliteOrbit { 
            semi_major_axis: calculate_semi_major_axis(value.mean_motion) as f32, 
            eccentricity: value.eccentricity as f32, 
            inclination: value.inclination as f32, 
            raan: value.right_ascension as f32, 
            argument_of_perigee: value.argument_of_perigee as f32, 
            true_anomaly: 0.0, 
            epoch: 0.0 
        }
    }
}

fn calculate_semi_major_axis(mean_motion_revs_per_day: f64) -> f64 {
    // Constants
    const MU: f64 = 3.986004418e14; // Gravitational parameter (m^3/s^2)
    const SECONDS_PER_DAY: f64 = 86400.0;
    
    // Convert mean motion from revolutions per day to radians per second
    let mean_motion_rad_per_sec = mean_motion_revs_per_day * (2.0 * std::f64::consts::PI) / SECONDS_PER_DAY;
    
    // Compute semi-major axis using Kepler's Third Law
    let semi_major_axis = (MU / mean_motion_rad_per_sec.powi(2)).powf(1.0 / 3.0);

    semi_major_axis / 1000.0
}


#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use approx::assert_abs_diff_eq;
    use bevy::{app::PanicHandlerPlugin, log::LogPlugin, prelude::*, state::app::StatesPlugin};
    use sgp4::Elements;
    use super::*;
    use crate::propagation::client::ConstFileClient;

    #[test]
    fn test_loading_of_celestial_elements() {

        let mut app = App::new();

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("assets");
        let client = ConstFileClient::new(d);

        app
            .add_plugins((MinimalPlugins, StatesPlugin, LogPlugin::default(), PanicHandlerPlugin, LoadElementsPlugin::<ConstFileClient>::new()))
            .insert_resource(client.clone());

        let mut writer = app.world_mut().resource_mut::<Events<LoadElements>>();
        writer.send(LoadElements { group: "galileo".to_owned(), format: "JSON".to_owned() });
        drop(writer);
        println!("Sent event");

        let mut res = vec![];
        for _ in 0..1000 {
            app.update();

            let result_events = app.world().resource::<Events<LoadedElements>>();
            let mut reader = result_events.get_reader();

            let mut read = reader.read(&result_events);
            if let Some(elements) = read.next() {
                res = elements.data.clone();
            }
        };

        println!("{:?}", display_elements(&res));

        for elems in &res {
            let orbit: SatelliteOrbit = elems.as_ref().into();
            assert_abs_diff_eq!(orbit.inclination, 56.0f32.to_radians(), epsilon = 8.0f32.to_radians());
        }

        assert!(!res.is_empty());
    }

    #[test]
    fn test_propagation_logic() {
        let mut app = App::new();

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("assets");
        let client = ConstFileClient::new(d);

        app
            .add_plugins((MinimalPlugins, StatesPlugin, LogPlugin::default(), PanicHandlerPlugin, LoadElementsPlugin::<ConstFileClient>::new(), PropagateElementsPlugin))
            .insert_resource(client.clone());

        let mut writer = app.world_mut().resource_mut::<Events<LoadElements>>();
        writer.send(LoadElements { group: "galileo".to_owned(), format: "JSON".to_owned() });
        drop(writer);

        let mut res = vec![];
        for _ in 0..1000 {
            app.update();

            let result_events = app.world().resource::<Events<LoadedElements>>();
            let mut reader = result_events.get_reader();

            let mut read = reader.read(&result_events);
            if let Some(elements) = read.next() {
                res = elements.data.clone();
            }
        };

        let mut data = vec![];
        for elements in &res {
            let elements = InGameElements(elements.clone());
            let entity = app.world_mut().spawn(elements.clone());
            data.push((entity.id(), elements));
        }
        let mut writer = app.world_mut().resource_mut::<Events<Propagate>>();
        writer.send(Propagate { data, dt_minutes: 30.0 });

        let mut res: Option<Propageted> = None;
        for _ in 0..1000 {
            app.update();
            let result_events = app.world().resource::<Events<Propageted>>();
            let mut reader = result_events.get_reader();
            let mut read = reader.read(&result_events);
            if let Some(propageted) = read.next() {
                res = Some(propageted.clone());
            }
        }

        if let Some(res) = res {
            println!("{:?}", res);
        } else {
            panic!("Failed no event");
        }
    }

    fn display_elements(elements: &Vec<Arc<Elements>>) -> String {
        let res: Vec<_> = elements.iter().map(|els| format!("object_name={:?},international_designator={:?},norad_id={},classification={:?},datetime={:?},inclination={}", els.object_name, els.international_designator, els.norad_id, display_clasification(&els), els.datetime, els.inclination)).collect();
        res.join("\n")
    }

    fn display_clasification(elem: &Elements) -> String {
        match elem.classification {
            sgp4::Classification::Unclassified => "unclassified".to_owned(),
            sgp4::Classification::Classified => "classified".to_owned(),
            sgp4::Classification::Secret => "secret".to_owned(),
        }
    }
}