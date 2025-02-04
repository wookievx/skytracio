use std::marker::PhantomData;

use bevy::{gltf::GltfMesh, math::Vec3A, prelude::*, render::primitives::Aabb};

pub struct LoadAndScaleEarthModelPlugin<T> {
    pub target_in_game_radius: f32,
    phantom_data: PhantomData<T>    
}

impl <T> LoadAndScaleEarthModelPlugin<T> {
    pub fn new(target_in_game_diameter: f32) -> Self {
        Self {
            target_in_game_radius: target_in_game_diameter,
            phantom_data: PhantomData
        }
    }
}

#[derive(Event)]
pub struct AssetPrepared {
    pub entity_id: Entity
}

//because bevy have strange limitations, need to do it like that
#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum InternalState {
    #[default]
    Loading,
    Loaded,
    Done
}

#[derive(Resource)]
struct ScaleResource {
    target_in_game_radius: f32,
    spawned_earth: Option<Entity>
}

impl <T: Component + Default> Plugin for LoadAndScaleEarthModelPlugin<T> {
    fn build(&self, app: &mut App) {
        app
          .add_event::<AssetPrepared>()
          .init_state::<InternalState>()
          .insert_resource(ScaleResource { target_in_game_radius: self.target_in_game_radius, spawned_earth: None })
          .add_systems(Startup, EarthAssets::load_model)
          .add_systems(Update, EarthAssets::transition_to_loaded.run_if(in_state(InternalState::Loading)))
          .add_systems(OnEnter(InternalState::Loaded), LoadedEarthAssets::spawn_earth_system::<T>)
          .add_systems(Update, LoadedEarthAssets::adjust_earth_size_and_mark_done::<T>.run_if(in_state(InternalState::Loaded)))
          .add_systems(Update, LoadedEarthAssets::debug_earth.run_if(in_state(InternalState::Loaded)));        
    }
}



#[derive(Resource)]
pub struct EarthAssets {
    assets: Handle<Gltf>
}

impl EarthAssets {
    fn load_model(mut commands: Commands, ass: Res<AssetServer>) {
        let assets = ass.load("3d/Earth_1_12756.glb");
        commands.insert_resource(Self { assets });
    }

    fn transition_to_loaded(mut commands: Commands, state: Res<State<InternalState>>, mut next_state: ResMut<NextState<InternalState>>, res: Res<EarthAssets>, server: Res<AssetServer>) {
        if let Some(loaded_assets) = res.get_loaded_assets(server) {
            match state.get() {
                InternalState::Loading => next_state.set(InternalState::Loaded),
                _ => unreachable!()
            }
            commands.insert_resource(loaded_assets);
        }
    }

    fn get_loaded_assets(
        &self,
        server: Res<AssetServer>,
    ) -> Option<LoadedEarthAssets> {
        use bevy::asset::LoadState;

        match server.get_load_state(&self.assets) {
            Some(LoadState::Loaded) => Some(LoadedEarthAssets::build(self)),
            _ => None,
        }
    }
}

//should only be constructed if assets were loaded
#[derive(Resource)]
pub struct LoadedEarthAssets {
    assets: Handle<Gltf>,
}

impl LoadedEarthAssets {
    fn build(from: &EarthAssets) -> LoadedEarthAssets {
        LoadedEarthAssets {
            assets: from.assets.clone(),
        }
    }

    fn spawn_earth_system<T: Component + Default>(
        assets: Res<LoadedEarthAssets>,
        scale_resource: ResMut<ScaleResource>,
        commands: Commands, 
        gltf_assets: Res<Assets<Gltf>>, 
        mesh_assets: Res<Assets<GltfMesh>>
    ) {
        assets.debug_earth_scene::<T>(scale_resource, commands, gltf_assets, mesh_assets);
    }

    fn debug_earth_scene<T: Component + Default>(&self, mut scale_resource: ResMut<ScaleResource>, mut commands: Commands, gltf_assets: Res<Assets<Gltf>>, mesh_assets: Res<Assets<GltfMesh>>) {
        let assets = gltf_assets.get(&self.assets).unwrap();
        for mesh in &assets.meshes {
            if let Some(mesh) = mesh_assets.get(mesh) {
                for primitive in &mesh.primitives {
                    println!("Got {:?}", primitive);
                }
            }
        }

        scale_resource.spawned_earth = Some(
            commands.spawn(SceneBundle {
                scene: assets.scenes[0].clone(),
                ..default()
            })
            .insert(<T>::default())
            .id()
        );
        println!("Got earth id: {:?}", scale_resource.spawned_earth);
    }

    fn debug_earth(
        world: &World,
        resource: Res<ScaleResource>,
        children: Query<&Children>
    ) {
        fn recursive(
            entity: Entity, 
            world: &World,
            resource: &Res<ScaleResource>,
            children: &Query<&Children>
        ) {
            let components = world.inspect_entity(entity);
            println!("For entity: {entity}");
            for c in components {
                println!("{:?}", c);
            }
            let children_val = match children.get(entity) {
                Ok(children_val)  => children_val,
                Err(error) => {
                    warn!("Got error querying children: {:?}", error);
                    return;
                }
            };
            for child in children_val {
                recursive(*child, world, resource, children);
            }
        }
        if let Some(earth) = resource.spawned_earth {
            recursive(earth, world, &resource, &children);
        };
    }

    fn adjust_earth_size_and_mark_done<T: Component + Default>(
        mut ev_done: EventWriter<AssetPrepared>,
        mut next_state: ResMut<NextState<InternalState>>,
        resource: Res<ScaleResource>,
        mut scene: Query<&mut Transform, With<T>>,
        children: Query<&Children>,
        meshes: Query<Option<&Aabb>, With<Handle<Mesh>>>
    ) {

        let mut aabbs: Vec<_> = vec![];

        let Some(entity) = resource.spawned_earth else {
            return;
        };
        
        Self::recursive_query_for_meshes(&mut aabbs, entity, &children, &meshes);
        if aabbs.is_empty() {
           return;
        }
        println!("Got aabs: {:?}", aabbs);
        
        let mut min = Vec3A::splat(f32::MAX);
        let mut max = Vec3A::splat(f32::MIN);
        for aabb in &aabbs {
            // If the Aabb had not been rotated, applying the non-uniform scale would produce the
            // correct bounds. However, it could very well be rotated and so we first convert to
            // a Sphere, and then back to an Aabb to find the conservative min and max points.
            // let sphere = Sphere {
            //     center: Vec3A::from(transform.transform_point(Vec3::from(aabb.center))),
            //     radius: transform.radius_vec3a(aabb.half_extents),
            // };
            // assuming sphere
            min = min.min(aabb.min());
            max = max.max(aabb.max());
        }

        let size = (max.x - min.x).abs();

        let scale = Vec3::splat(resource.target_in_game_radius / size);

        for mut scene_transform in scene.iter_mut() {
            scene_transform.scale = scale;
            scene_transform.rotation = Quat::from_rotation_x(std::f32::consts::PI / 2.0);
        }

        ev_done.send(AssetPrepared { entity_id: resource.spawned_earth.expect("earth instance must be present here") });
        next_state.set(InternalState::Done);
    }

    fn recursive_query_for_meshes(builder: &mut Vec<Aabb>, entity: Entity, children: &Query<&Children>, meshes: &Query<Option<&Aabb>, With<Handle<Mesh>>>) {
        let aabb = match meshes.get(entity) {
            Ok(aabb) => aabb,
            Err(error) => {
                info!("Got error querying aab: {:?}", error);
                None
            }
        };
        if let Some(aabb) = aabb {
            builder.push(*aabb);
        }
        let children_val = match children.get(entity) {
            Ok(children_val)  => children_val,
            Err(error) => {
                info!("Got error querying children: {:?}", error);
                return;
            }
        };
        for child in children_val {
            Self::recursive_query_for_meshes(builder, *child, children, meshes);          
        }
    }

}