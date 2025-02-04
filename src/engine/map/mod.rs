//! This module manages map loading and generation in a Bevy-based game,
//! including wall creation and player spawn point selection for tank gameplay.
use std::{fs, mem, path::Path};

use bevy::{
    app::{Plugin, Startup, Update
    }, asset::{Asset, AssetApp, AssetServer, Assets, Handle}, color::LinearRgba, log::warn, math::Vec3, prelude::{
        in_state, AppExtStates, Camera2dBundle, Commands, Component, GlobalTransform, Image, InheritedVisibility, IntoSystemConfigs, Mesh, NextState, Res, ResMut, Resource, Transform, ViewVisibility, Visibility
    }, reflect::Reflect, sprite::Sprite
};
use bevy_rapier2d::prelude::Collider;
use gen_state::Step;
use map_loader::MapLoader;
use serde::{Deserialize, Serialize};

use crate::{engine::tank::gen::{create_minimal_tank, create_tank}, player::PlayerID};

use super::tank::material::TankMaterial;

pub mod map_loader;
pub mod gen_state;
pub type Coord = (usize, usize);

/// Represents a game map with dimensions, walls, and spawn points for tanks.
/// 
/// # Fields
/// - `dim`: The dimensions of the map as a tuple of width and height.
/// - `walls`: A vector of coordinates representing the positions of walls on the map.
/// - `spawn_points`: A vector of coordinates representing possible spawn points for tanks.
#[derive(Debug, Clone, Asset, Reflect, Deserialize, Serialize)]
pub struct Map{
    pub dim: (usize, usize),
    pub walls: Vec<Coord>,
    pub spawn_points: Vec<Coord>
}

/// Holds the current map being used in the game, referenced by its asset handle.
/// 
/// # Fields
/// - `0`: An optional handle to the current map asset.
#[derive(Debug, Clone, Resource)]
pub enum CurrentMap{
    None,
    AssetPath(String),
    Handle(Handle<Map>)
}

impl Default for CurrentMap {
    fn default() -> Self {
        CurrentMap::None
    }
}

pub const WALL_SIZE: f32 = 32.;

/// A component representing a wall in the game.
#[derive(Debug, Clone, Copy, Component)]
pub struct Wall;

/// Loads the specified map from the asset server and sets it as the current map.
/// 
/// # Parameters
/// - `asset_server`: The asset server resource for loading map assets.
/// - `current_map`: The current map resource to store the loaded map.
/// - `next_state`: A mutable reference to the next state in the game state management.
pub fn load_map(
    asset_server: Res<AssetServer>,

    mut current_map: ResMut<CurrentMap>,
    mut next_state: ResMut<NextState<Step>>
){
    let current_map = current_map.as_mut();
    let mut selected_map = match &current_map {
        CurrentMap::None => {
            let map_folder: Vec<String> = fs::read_dir("assets/maps")
                .expect("Unable to load \"assets/maps\"")
                .into_iter()
                .filter(|file| file.is_ok())
                .map(|file| file.unwrap())
                .map(|file| format!("{}", file.file_name().to_str().unwrap()))
                .collect();

            let i: usize = rand::random::<usize>();

            CurrentMap::Handle(
                asset_server.load(
                    format!("maps/{}", map_folder[i % map_folder.len()])
                )
            )
        },
        CurrentMap::AssetPath(map_name) => {
            if !Path::new(&format!("assets/maps/{}", map_name)).exists() {
                warn!("Map does not exist ({})", map_name);
                panic!();
            }

            CurrentMap::Handle(
                asset_server.load(
                    format!("maps/{}", map_name)
                )
            )
        },
        CurrentMap::Handle(_) => panic!("Invalid state when loading a map"),
    };
    
    mem::swap(current_map, &mut selected_map);

    next_state.set(Step::GenerateMap);
}

/// Generates a minimal map by spawning walls and two tanks at random spawn points.
/// 
/// # Parameters
/// - `commands`: The command buffer for spawning entities.
/// - `current_map`: The current map resource containing the loaded map.
/// - `maps`: The resource containing all loaded maps.
/// - `next_state`: A mutable reference to the next state in the game state management.
pub fn generate_minimal_map(
    mut commands: Commands,
    
    current_map: Res<CurrentMap>,
    maps: Res<Assets<Map>>,
    
    mut next_state: ResMut<NextState<Step>>
){
    let current_map = current_map.as_ref();
    let CurrentMap::Handle(map_id) = current_map else {
        todo!()
    };
    let map_id = map_id.id();

    let Some(map) = maps.as_ref().get(map_id) else {
        return;
    };

    #[cfg(feature = "debug")]
    println!("{map:?}");
    // generate walls & pick spawn points
    type WallBundle = (Wall,Transform,GlobalTransform, Collider);
    let walls: Vec<WallBundle> = map.walls.iter()
        .map(|(x, y)| (*x as f32, *y as f32))
        .map( |(x, y)| (
            Wall,
            Transform{
                translation: Vec3{
                    x: x * WALL_SIZE,
                    y: y * WALL_SIZE,
                    z: 0.,
                },
                ..Default::default()
            },
            Default::default(),
            Collider::cuboid(WALL_SIZE/8., WALL_SIZE/8.)
        ))
        .collect();
    commands.spawn_batch(
        walls
    );

    let p1_spawn = {
        let i1 = rand::random::<usize>() % map.spawn_points.len();

        &map.spawn_points[i1]
    };

    let p2_spawn = map.spawn_points
        .iter()
        .filter(|&&point| point != *p1_spawn)
        .fold(
            (*p1_spawn, 0isize),
            |acc, next| {
                let dist = (next.0 as isize - p1_spawn.0 as isize).pow(2) + (next.1 as isize - p1_spawn.1 as isize).pow(2);
                
                match dist > acc.1 {
                    true => (*next, dist),
                    false => acc
                }
            }
        ).0;

    {
        let p1 = create_minimal_tank(
            p1_spawn.0 as f32 * WALL_SIZE,
            p1_spawn.1 as f32 * WALL_SIZE,
            0,
            &mut commands
        );
        commands.entity(p1)
            .insert(PlayerID::<0>);
    }

    {
        let p2 = create_minimal_tank(
            p2_spawn.0 as f32 * WALL_SIZE,
            p2_spawn.1 as f32 * WALL_SIZE,
            1,
            &mut commands
        );
        commands.entity(p2)
            .insert(PlayerID::<1>);
    }

    next_state.set(Step::Finished);
}

/// Generates a complete map by spawning walls, a camera, and two tanks at random spawn points.
/// 
/// # Parameters
/// - `commands`: The command buffer for spawning entities.
/// - `current_map`: The current map resource containing the loaded map.
/// - `maps`: The resource containing all loaded maps.
/// - `asset_server`: The asset server resource for loading textures.
/// - `next_state`: A mutable reference to the next state in the game state management.
pub fn generate_map(
    mut commands: Commands,
    
    current_map: Res<CurrentMap>,
    maps: Res<Assets<Map>>,

    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TankMaterial>>,
    
    mut next_state: ResMut<NextState<Step>>
){
    let current_map = current_map.as_ref();
    let CurrentMap::Handle(map_id) = current_map else {
        todo!()
    };
    let map_id = map_id.id();

    let Some(map) = maps.as_ref().get(map_id) else {
        return;
    };

    // println!("{map:?}");
    // generate walls & pick spawn points
    type WallBundle = (
        Wall,
        Transform,
        GlobalTransform,

        Collider,

        Sprite,
        Handle<Image>,
        Visibility,
        InheritedVisibility,
        ViewVisibility,
    );
    let walls: Vec<WallBundle> = map.walls.iter()
        .map(|(x, y)| (*x as f32, *y as f32))
        .map( |(x, y)| (
            Wall,
            Transform{
                translation: Vec3{
                    x: x * WALL_SIZE,
                    y: y * WALL_SIZE,
                    z: 0.,
                },
                ..Default::default()
            },
            Default::default(),

            Collider::cuboid(WALL_SIZE/2., WALL_SIZE/2.),
            
            Default::default(),
            asset_server.load("textures\\map\\wall.png"),
            Default::default(),
            Default::default(),
            Default::default()
        ))
        .collect();
    commands.spawn_batch(
        walls
    );

    commands.spawn((
        Camera2dBundle{
            transform: Transform{
                translation: Vec3 {
                    x: map.dim.0 as f32 / 2. * WALL_SIZE,
                    y: map.dim.1 as f32 / 2. * WALL_SIZE,
                    z: 1. 
                },
                ..Default::default()
            },
            ..Default::default()
        },
    ));

    
    let p1_spawn = {
        let i1 = rand::random::<usize>() % map.spawn_points.len();

        &map.spawn_points[i1]
    };

    let p2_spawn = map.spawn_points
        .iter()
        .filter(|&&point| point != *p1_spawn)
        .fold(
            (*p1_spawn, 0isize),
            |acc, next| {
                let dist = (next.0 as isize - p1_spawn.0 as isize).pow(2) + (next.1 as isize - p1_spawn.1 as isize).pow(2);
                
                match dist > acc.1 {
                    true => (*next, dist),
                    false => acc
                }
            }
        ).0;
    {
        let p1 = create_tank(
            p1_spawn.0 as f32 * WALL_SIZE,
            p1_spawn.1 as f32 * WALL_SIZE,
            0,
            LinearRgba::new(1., 0., 0., 1.),
            &mut commands,
            &mut meshes,
            &mut materials,
            &asset_server
        );
        commands.entity(p1)
            .insert(PlayerID::<0>);
    }

    {
        let p2 = create_tank(
            p2_spawn.0 as f32 * WALL_SIZE,
            p2_spawn.1 as f32 * WALL_SIZE,
            1,
            LinearRgba::new(0., 0., 1., 1.),
            &mut commands,
            &mut meshes,
            &mut materials,
            &asset_server
        );
        commands.entity(p2)
            .insert(PlayerID::<1>);
    }


    next_state.set(Step::Finished);
}

/// A Bevy plugin for managing map loading and generation.
/// 
/// # Fields
/// - `bool`: A flag indicating whether to generate a minimal map(headless) or a complete map.
pub struct MapPlugin(pub bool, pub Option<String>);

impl Plugin for MapPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .init_asset::<Map>()
            .init_asset_loader::<MapLoader>()


            .init_state::<Step>();

            match &self.1{
                Some(selected_map) => {
                    app.insert_resource(CurrentMap::AssetPath(selected_map.clone()));
                },
                None => {
                    app.init_resource::<CurrentMap>();
                },
            };

            app.add_systems(
                    Startup,
                    load_map.run_if(in_state(Step::LoadMap))
                );
            
        match self.0 {
            false => {
                app.add_systems(
                    Update,
                    (
                        generate_minimal_map.run_if(in_state(Step::GenerateMap)),
                    )
                );
            },
            true => {
                app.add_systems(
                    Update,
                    (
                        generate_map.run_if(in_state(Step::GenerateMap)),
                    )
                );
            }
        }
    }
}