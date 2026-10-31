use super::{
    endless_maze::{MazeLayout, arc_position, attach_hook_shape, distance_to_segment},
    void_level::{FireHero, spawn_fire_hero},
};
use crate::ProjectFiles;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::Deserialize;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::fs;

const RING_TIERS: usize = 72;
const RING_HEIGHT: f32 = 0.92;
const PORTAL_RADIUS: f32 = 0.62;
const RING_CATCH_RADIUS: f32 = 0.72;
const HOOK_SPEED: f32 = 15.0;
const HOOK_CATCH_RADIUS: f32 = 0.8;
const HOOK_MUZZLE_HEIGHT: f32 = 0.62;
const CHAIN_LINKS: usize = 24;
const MAX_SAVED_RINGS: usize = 64;
const SAVE_INTERVAL: f32 = 1.5;
const SAVE_MAGIC: &[u8; 4] = b"RVP6";
const SAVE_VERSION: u8 = 1;
#[cfg(target_arch = "wasm32")]
const BROWSER_SAVE_KEY: &str = "ringen.level-006.world.v1";

#[derive(Deserialize, Clone)]
struct WorldConfig {
    ring_population: usize,
    maze_schemas: Vec<MazeSchema>,
}

#[derive(Deserialize, Clone)]
struct MazeSchema {
    id: u8,
    name: String,
    seed: u64,
    cells: usize,
    extra_passages: usize,
    portals: Vec<PortalEndpoint>,
}

#[derive(Deserialize, Clone)]
struct PortalEndpoint {
    cell: [usize; 2],
    destination_maze: u8,
    destination_portal: usize,
}

#[derive(Resource)]
pub(crate) struct PortalWorld {
    schemas: Vec<MazeSchema>,
    active_maze: u8,
    layout: MazeLayout,
    rings: Vec<TravelRing>,
    next_ring_id: u32,
    caught_total: u32,
    caught_by_tier: [u32; RING_TIERS],
    last_caught_tier: u8,
    ring_population: usize,
    player_position: Vec3,
    player_portal_cooldown: f32,
    rng_state: u64,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    save_path: PathBuf,
    save_timer: f32,
    dirty: bool,
    force_save: bool,
}

#[derive(Resource)]
pub(crate) struct PortalAssets {
    floor_mesh: Handle<Mesh>,
    wall_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    portal_mesh: Handle<Mesh>,
    hook_segment_mesh: Handle<Mesh>,
    chain_link_mesh: Handle<Mesh>,
    aim_marker_mesh: Handle<Mesh>,
    floor_materials: [Handle<StandardMaterial>; 2],
    wall_material: Handle<StandardMaterial>,
    ring_materials: [Handle<StandardMaterial>; 4],
    portal_materials: [Handle<StandardMaterial>; 2],
    hook_material: Handle<StandardMaterial>,
    chain_material: Handle<StandardMaterial>,
    aim_marker_material: Handle<StandardMaterial>,
}

/// Where the mouse cursor points on the ring-height plane, or `None` when it is outside the window.
#[derive(Resource, Default)]
pub(crate) struct PortalHookAim {
    target: Option<Vec3>,
}

/// The hook is a tiny state machine: it rests, flies out along an arc, then reels back in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HookPhase {
    Idle,
    Flying,
    Returning,
}

#[derive(Component)]
pub(crate) struct PortalHook {
    phase: HookPhase,
    maze_id: u8,
    start: Vec3,
    target: Vec3,
    progress: f32,
    duration: f32,
}

#[derive(Component)]
pub(crate) struct PortalHookLink {
    index: usize,
}

#[derive(Component)]
pub(crate) struct PortalAimMarker;

/// Tags the hook, its chain, and the aim marker. They are not `PortalSceneObject`s, so they
/// survive portal travel and are removed only when the level closes.
#[derive(Component)]
pub(crate) struct PortalHookGear;

#[derive(Component)]
pub(crate) struct PortalMazeStatus;

#[derive(Component)]
pub(crate) struct PortalSceneObject;

#[derive(Component)]
pub(crate) struct RingVisual {
    id: u32,
}

#[derive(Component)]
pub(crate) struct PortalVisual {
    index: usize,
    phase: f32,
}

#[derive(Clone)]
struct TravelRing {
    id: u32,
    tier: u8,
    maze_id: u8,
    position: Vec3,
    origin: Vec3,
    target: Vec3,
    speed: f32,
    growth: f32,
    phase: f32,
    progress: f32,
    duration: f32,
    hop_height: f32,
    portal_cooldown: f32,
    moving: bool,
    hopping: bool,
}

struct SavedWorld {
    active_maze: u8,
    next_ring_id: u32,
    caught_total: u32,
    caught_by_tier: [u32; RING_TIERS],
    last_caught_tier: u8,
    player_position: Vec3,
    rng_state: u64,
    rings: Vec<TravelRing>,
}

impl Default for SavedWorld {
    fn default() -> Self {
        Self {
            active_maze: 0,
            next_ring_id: 1,
            caught_total: 0,
            caught_by_tier: [0; RING_TIERS],
            last_caught_tier: 0,
            player_position: Vec3::ZERO,
            rng_state: 0xA076_1D64_78BD_642F,
            rings: Vec::new(),
        }
    }
}

impl PortalWorld {
    pub(crate) fn active_maze_id(&self) -> u8 {
        self.active_maze
    }

    pub(crate) fn camera_height(&self) -> f32 {
        13.0 + (self.layout.width as f32 * 0.11).min(5.0)
    }

    pub(crate) fn camera_depth(&self) -> f32 {
        11.0 + (self.layout.width as f32 * 0.09).min(4.0)
    }

    fn active_schema(&self) -> &MazeSchema {
        self.schemas
            .iter()
            .find(|schema| schema.id == self.active_maze)
            .expect("active maze schema must exist")
    }

    fn schema(&self, id: u8) -> &MazeSchema {
        self.schemas
            .iter()
            .find(|schema| schema.id == id)
            .expect("portal destination schema must exist")
    }

    /// Picks an open tile of the given maze that is clear of portals, the fire, and other rings.
    /// Inactive mazes are not drawn, so their layout is rebuilt from the schema for the lookup.
    fn pick_free_tile(&mut self, maze_id: u8) -> Vec3 {
        let is_active = maze_id == self.active_maze;
        let layout = if is_active {
            self.layout.clone()
        } else {
            layout_for(self.schema(maze_id))
        };
        let mut open_tiles = layout.open_tiles();
        let portal_positions = self
            .schema(maze_id)
            .portals
            .iter()
            .map(|portal| layout.world_position(portal_grid_cell(portal)))
            .collect::<Vec<_>>();
        let clear_tiles = open_tiles
            .iter()
            .copied()
            .filter(|position| {
                portal_positions
                    .iter()
                    .all(|portal| xz_distance(*position, *portal) > 1.5)
                    && (!is_active || xz_distance(*position, self.player_position) > 1.25)
                    && self
                        .rings
                        .iter()
                        .filter(|ring| ring.maze_id == maze_id)
                        .all(|ring| xz_distance(ring.position, *position) > 1.0)
            })
            .collect::<Vec<_>>();
        if !clear_tiles.is_empty() {
            open_tiles = clear_tiles;
        }
        open_tiles[random_index(&mut self.rng_state, open_tiles.len())]
    }

    fn spawn_ring_record_in(&mut self, maze_id: u8) -> TravelRing {
        let tier = roll_ring_tier(&mut self.rng_state);
        let position = self.pick_free_tile(maze_id);
        let speed =
            0.58 + (tier as f32 - 1.0) / 71.0 * 1.75 + random_unit(&mut self.rng_state) * 0.38;
        let ring = TravelRing {
            id: self.next_ring_id,
            tier,
            maze_id,
            position,
            origin: position,
            target: position,
            speed,
            growth: 1.0,
            phase: random_unit(&mut self.rng_state) * std::f32::consts::TAU,
            progress: 0.0,
            duration: 0.0,
            hop_height: 0.0,
            portal_cooldown: 0.0,
            moving: false,
            hopping: false,
        };
        self.next_ring_id = self.next_ring_id.wrapping_add(1).max(1);
        ring
    }

    fn rings_in(&self, maze_id: u8) -> usize {
        self.rings
            .iter()
            .filter(|ring| ring.maze_id == maze_id)
            .count()
    }

    /// Brings every maze up to `ring_population` rings so each world has something to chase.
    /// A maze that is short first borrows a ring from a maze that holds more than its share, which
    /// keeps the total steady when rings simply wandered through portals. Only when no maze has a
    /// spare does it create a new ring, which covers new games and saves made before rings were
    /// spread across mazes. The total never passes `MAX_SAVED_RINGS`.
    fn balance_rings(&mut self) {
        let maze_ids = self
            .schemas
            .iter()
            .map(|schema| schema.id)
            .collect::<Vec<_>>();
        for maze_id in maze_ids {
            while self.rings_in(maze_id) < self.ring_population {
                if let Some(index) = self.spare_ring_index(maze_id) {
                    let position = self.pick_free_tile(maze_id);
                    let ring = &mut self.rings[index];
                    ring.maze_id = maze_id;
                    ring.position = position;
                    ring.origin = position;
                    ring.target = position;
                    ring.progress = 0.0;
                    ring.duration = 0.0;
                    ring.portal_cooldown = 0.0;
                    ring.moving = false;
                    ring.hopping = false;
                } else if self.rings.len() < MAX_SAVED_RINGS {
                    let ring = self.spawn_ring_record_in(maze_id);
                    self.rings.push(ring);
                } else {
                    break;
                }
                self.dirty = true;
            }
        }
    }

    /// The last ring of whichever other maze holds the most rings above its share, if any does.
    fn spare_ring_index(&self, except: u8) -> Option<usize> {
        self.schemas
            .iter()
            .map(|schema| schema.id)
            .filter(|id| *id != except && self.rings_in(*id) > self.ring_population)
            .max_by_key(|id| self.rings_in(*id))
            .and_then(|id| self.rings.iter().rposition(|ring| ring.maze_id == id))
    }

    fn encode_binary(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(320 + self.rings.len() * 34);
        bytes.extend_from_slice(SAVE_MAGIC);
        bytes.push(SAVE_VERSION);
        bytes.push(self.active_maze);
        push_u32(&mut bytes, self.next_ring_id);
        push_u32(&mut bytes, self.caught_total);
        bytes.push(self.last_caught_tier);
        push_u64(&mut bytes, self.rng_state);
        push_fixed_i16(&mut bytes, self.player_position.x);
        push_fixed_i16(&mut bytes, self.player_position.z);
        push_u16(&mut bytes, self.rings.len().min(MAX_SAVED_RINGS) as u16);
        for count in self.caught_by_tier {
            push_u32(&mut bytes, count);
        }
        for ring in self.rings.iter().take(MAX_SAVED_RINGS) {
            push_u32(&mut bytes, ring.id);
            bytes.push(ring.tier);
            bytes.push(ring.maze_id);
            bytes.push(u8::from(ring.moving) | (u8::from(ring.hopping) << 1));
            push_fixed_i16(&mut bytes, ring.position.x);
            push_fixed_i16(&mut bytes, ring.position.z);
            push_fixed_i16(&mut bytes, ring.origin.x);
            push_fixed_i16(&mut bytes, ring.origin.z);
            push_fixed_i16(&mut bytes, ring.target.x);
            push_fixed_i16(&mut bytes, ring.target.z);
            push_fixed_u16(&mut bytes, ring.speed);
            push_fixed_u16(&mut bytes, ring.growth);
            push_fixed_u16(&mut bytes, ring.phase);
            push_u16(
                &mut bytes,
                (ring.progress.clamp(0.0, 1.0) * u16::MAX as f32) as u16,
            );
            push_fixed_u16(&mut bytes, ring.duration);
            push_fixed_u16(&mut bytes, ring.hop_height);
            push_fixed_u16(&mut bytes, ring.portal_cooldown);
        }
        bytes
    }
}

pub(crate) fn spawn_portal_hero(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_fire_hero(&mut commands, &mut meshes, &mut materials);
}

pub(crate) fn start_portal_maze_scene(
    mut commands: Commands,
    project_files: Res<ProjectFiles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut heroes: Query<&mut Transform, With<FireHero>>,
) {
    let mut config = serde_json::from_str::<WorldConfig>(include_str!(
        "../../data/levels/level-006-schemas.json"
    ))
    .unwrap_or_else(|error| {
        warn!("Level 6 schema data is invalid; using the fallback worlds: {error}");
        WorldConfig::fallback()
    });
    if !config_is_valid(&config) {
        warn!("Level 6 schema links are invalid; using the fallback worlds");
        config = WorldConfig::fallback();
    }

    let save_path = project_files.root.join("data/saves/level-006-world.bin");
    let saved = load_saved_world(&save_path, config.maze_schemas.len()).unwrap_or_default();
    let active_maze = if config
        .maze_schemas
        .iter()
        .any(|schema| schema.id == saved.active_maze)
    {
        saved.active_maze
    } else {
        config.maze_schemas[0].id
    };
    let active_schema = config
        .maze_schemas
        .iter()
        .find(|schema| schema.id == active_maze)
        .expect("validated configuration always has a first maze");
    let layout = layout_for(active_schema);
    let ring_population = config
        .ring_population
        .clamp(1, MAX_SAVED_RINGS / config.maze_schemas.len());
    let mut world = PortalWorld {
        schemas: config.maze_schemas,
        active_maze,
        layout,
        rings: saved.rings,
        next_ring_id: saved.next_ring_id.max(1),
        caught_total: saved.caught_total,
        caught_by_tier: saved.caught_by_tier,
        last_caught_tier: saved.last_caught_tier.min(RING_TIERS as u8),
        ring_population,
        player_position: saved.player_position,
        player_portal_cooldown: 0.0,
        rng_state: saved.rng_state.max(1),
        save_path,
        save_timer: 0.0,
        dirty: false,
        force_save: true,
    };
    if !world.layout.can_stand(world.player_position) {
        world.player_position = Vec3::ZERO;
    }
    for ring in &mut world.rings {
        if ring.maze_id == world.active_maze && !world.layout.can_stand(ring.position) {
            let nearest = world
                .layout
                .open_tiles()
                .into_iter()
                .min_by(|a, b| {
                    xz_distance(*a, ring.position).total_cmp(&xz_distance(*b, ring.position))
                })
                .unwrap_or(Vec3::ZERO);
            ring.position = nearest;
            ring.origin = nearest;
            ring.target = nearest;
            ring.moving = false;
            ring.hopping = false;
        }
    }
    world.balance_rings();
    for mut hero in &mut heroes {
        hero.translation = world.player_position;
    }

    let assets = create_assets(&mut meshes, &mut materials);
    spawn_active_maze(&mut commands, &world, &assets);
    spawn_portal_hook(&mut commands, &assets);
    commands.insert_resource(assets);
    commands.insert_resource(world);
    commands.insert_resource(PortalHookAim::default());
}

fn spawn_portal_hook(commands: &mut Commands, assets: &PortalAssets) {
    for index in 0..CHAIN_LINKS {
        commands.spawn((
            PortalHookGear,
            PortalHookLink { index },
            Mesh3d(assets.chain_link_mesh.clone()),
            MeshMaterial3d(assets.chain_material.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }

    let hook = commands
        .spawn((
            PortalHookGear,
            PortalHook {
                phase: HookPhase::Idle,
                maze_id: 0,
                start: Vec3::ZERO,
                target: Vec3::ZERO,
                progress: 0.0,
                duration: 0.0,
            },
            Transform::from_xyz(0.0, HOOK_MUZZLE_HEIGHT, 0.0),
            Visibility::Hidden,
        ))
        .id();
    attach_hook_shape(
        commands,
        hook,
        &assets.hook_segment_mesh,
        &assets.hook_material,
    );

    commands.spawn((
        PortalHookGear,
        PortalAimMarker,
        Mesh3d(assets.aim_marker_mesh.clone()),
        MeshMaterial3d(assets.aim_marker_material.clone()),
        Transform::from_xyz(0.0, RING_HEIGHT, 0.0),
        Visibility::Hidden,
    ));
}

fn create_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> PortalAssets {
    PortalAssets {
        floor_mesh: meshes.add(Cuboid::new(0.98, 0.12, 0.98)),
        wall_mesh: meshes.add(Cuboid::new(1.0, 1.35, 1.0)),
        ring_mesh: meshes.add(Torus::new(0.31, 0.48)),
        portal_mesh: meshes.add(Torus::new(0.085, 0.55)),
        hook_segment_mesh: meshes.add(Cylinder::new(0.035, 1.0)),
        chain_link_mesh: meshes.add(Torus::new(0.017, 0.052)),
        aim_marker_mesh: meshes.add(Sphere::new(0.095)),
        floor_materials: [
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.07, 0.105, 0.16),
                perceptual_roughness: 0.94,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.105, 0.15, 0.22),
                perceptual_roughness: 0.94,
                ..default()
            }),
        ],
        wall_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.3, 0.41),
            metallic: 0.14,
            perceptual_roughness: 0.68,
            ..default()
        }),
        ring_materials: [
            materials.add(glowing_material(
                Color::srgb(0.35, 0.82, 1.0),
                LinearRgba::rgb(1.4, 5.0, 8.0),
            )),
            materials.add(glowing_material(
                Color::srgb(0.42, 1.0, 0.55),
                LinearRgba::rgb(1.8, 7.0, 2.4),
            )),
            materials.add(glowing_material(
                Color::srgb(1.0, 0.6, 0.18),
                LinearRgba::rgb(8.0, 3.0, 0.5),
            )),
            materials.add(glowing_material(
                Color::srgb(1.0, 0.3, 0.88),
                LinearRgba::rgb(8.0, 1.2, 6.0),
            )),
        ],
        portal_materials: [
            materials.add(glowing_material(
                Color::srgb(0.2, 0.85, 1.0),
                LinearRgba::rgb(1.0, 6.0, 12.0),
            )),
            materials.add(glowing_material(
                Color::srgb(0.92, 0.32, 1.0),
                LinearRgba::rgb(7.0, 1.4, 10.0),
            )),
        ],
        hook_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.94, 0.68, 0.3),
            metallic: 0.8,
            perceptual_roughness: 0.22,
            ..default()
        }),
        chain_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.73, 0.8),
            metallic: 0.75,
            perceptual_roughness: 0.25,
            ..default()
        }),
        aim_marker_material: materials.add(glowing_material(
            Color::srgb(0.35, 0.9, 1.0),
            LinearRgba::rgb(2.2, 8.0, 10.0),
        )),
    }
}

fn glowing_material(color: Color, emissive: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive,
        metallic: 0.55,
        perceptual_roughness: 0.24,
        ..default()
    }
}

fn spawn_active_maze(commands: &mut Commands, world: &PortalWorld, assets: &PortalAssets) {
    let layout = &world.layout;
    for y in 0..layout.width {
        for x in 0..layout.width {
            let position = layout.world_position((x, y));
            commands.spawn((
                PortalSceneObject,
                Mesh3d(assets.floor_mesh.clone()),
                MeshMaterial3d(assets.floor_materials[(x + y) % 2].clone()),
                Transform::from_translation(position + Vec3::Y * -0.14),
            ));
            if !layout.is_walkable(x, y) {
                commands.spawn((
                    PortalSceneObject,
                    Mesh3d(assets.wall_mesh.clone()),
                    MeshMaterial3d(assets.wall_material.clone()),
                    Transform::from_translation(position + Vec3::Y * 0.67),
                ));
            }
        }
    }

    let schema = world.active_schema();
    for (index, endpoint) in schema.portals.iter().enumerate() {
        let position = layout.world_position(portal_grid_cell(endpoint));
        commands.spawn((
            PortalSceneObject,
            PortalVisual {
                index,
                phase: index as f32 * 1.8,
            },
            Mesh3d(assets.portal_mesh.clone()),
            MeshMaterial3d(assets.portal_materials[index % assets.portal_materials.len()].clone()),
            Transform {
                translation: position + Vec3::Y * 0.84,
                rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ..default()
            },
        ));
        commands.spawn((
            PortalSceneObject,
            PointLight {
                color: if index % 2 == 0 {
                    Color::srgb(0.2, 0.8, 1.0)
                } else {
                    Color::srgb(0.88, 0.3, 1.0)
                },
                intensity: 650.0,
                range: 4.2,
                ..default()
            },
            Transform::from_translation(position + Vec3::Y * 0.82),
        ));
    }

    for ring in world
        .rings
        .iter()
        .filter(|ring| ring.maze_id == world.active_maze)
    {
        spawn_ring_visual(commands, ring, assets);
    }
}

fn spawn_ring_visual(commands: &mut Commands, ring: &TravelRing, assets: &PortalAssets) {
    let position = ring_render_position(ring);
    commands.spawn((
        PortalSceneObject,
        RingVisual { id: ring.id },
        Mesh3d(assets.ring_mesh.clone()),
        MeshMaterial3d(assets.ring_materials[ring_material_index(ring.tier)].clone()),
        Transform {
            translation: position,
            rotation: Quat::from_rotation_x(0.28),
            scale: Vec3::splat(ring_scale(ring)),
        },
    ));
}

pub(crate) fn move_portal_hero(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut world: ResMut<PortalWorld>,
    mut heroes: Query<(&FireHero, &mut Transform)>,
) {
    let mut direction = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        direction.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        direction.z += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }
    let delta = direction * time.delta_secs();
    for (hero, mut transform) in &mut heroes {
        let mut candidate = transform.translation;
        candidate.x += delta.x * hero.speed;
        if world.layout.can_stand(candidate) {
            transform.translation.x = candidate.x;
        }
        candidate = transform.translation;
        candidate.z += delta.z * hero.speed;
        if world.layout.can_stand(candidate) {
            transform.translation.z = candidate.z;
        }
        if transform.translation != world.player_position {
            world.player_position = transform.translation;
            world.dirty = true;
        }
    }
    if world.player_portal_cooldown > 0.0 {
        let still_inside_portal = world.active_schema().portals.iter().any(|portal| {
            xz_distance(
                world.player_position,
                world.layout.world_position(portal_grid_cell(portal)),
            ) <= PORTAL_RADIUS + 0.25
        });
        if !still_inside_portal {
            world.player_portal_cooldown = 0.0;
        }
    }
}

pub(crate) fn advance_traveling_rings(
    mut commands: Commands,
    time: Res<Time>,
    mut world: ResMut<PortalWorld>,
    assets: Res<PortalAssets>,
    mut visuals: Query<(Entity, &RingVisual, &mut Transform)>,
) {
    let delta_time = time.delta_secs().min(0.08);
    let open_tiles = world.layout.open_tiles();
    let active_maze = world.active_maze;
    let portals = world.active_schema().portals.clone();
    let schemas = world.schemas.clone();
    let layout = world.layout.clone();
    let mut ring_states = std::mem::take(&mut world.rings);
    let mut rng_state = world.rng_state;
    let mut remove_visuals = Vec::new();
    let mut did_change = false;
    let mut force_save = false;

    for ring in &mut ring_states {
        if ring.maze_id != active_maze {
            continue;
        }
        ring.phase = (ring.phase + delta_time * ring.speed * 1.7) % std::f32::consts::TAU;
        ring.portal_cooldown = (ring.portal_cooldown - delta_time).max(0.0);

        if !ring.moving {
            let neighbors = walkable_neighbors(&layout, ring.position);
            let wants_hop = random_index(&mut rng_state, 5) == 0;
            let hop_target = if wants_hop {
                let distant = open_tiles
                    .iter()
                    .copied()
                    .filter(|point| {
                        xz_distance(*point, ring.position) >= 2.2
                            && xz_distance(*point, ring.position) <= 7.0
                    })
                    .collect::<Vec<_>>();
                if distant.is_empty() {
                    None
                } else {
                    Some(distant[random_index(&mut rng_state, distant.len())])
                }
            } else {
                None
            };
            let target = match hop_target {
                Some(target) => Some(target),
                None if neighbors.is_empty() => None,
                None => Some(neighbors[random_index(&mut rng_state, neighbors.len())]),
            };
            if let Some(target) = target {
                ring.origin = ring.position;
                ring.target = target;
                ring.progress = 0.0;
                ring.hopping = hop_target.is_some();
                ring.hop_height = if ring.hopping { 2.05 } else { 0.0 };
                let movement_speed = ring.speed * if ring.hopping { 1.25 } else { 1.0 };
                ring.duration = (xz_distance(ring.position, target) / movement_speed).max(0.25);
                ring.moving = true;
                did_change = true;
            }
        }

        if ring.moving {
            let previous = ring.position;
            ring.progress = (ring.progress + delta_time / ring.duration.max(0.001)).min(1.0);
            ring.position = ring.origin.lerp(ring.target, ring.progress);
            did_change = true;
            if ring.progress >= 1.0 {
                ring.position = ring.target;
                ring.moving = false;
                ring.hopping = false;
                ring.growth = (ring.growth + 0.015).min(1.75);
            }

            if ring.portal_cooldown <= 0.0
                && let Some(endpoint) = portals.iter().find(|portal| {
                    let portal_position = layout.world_position(portal_grid_cell(portal));
                    xz_segment_distance(portal_position, previous, ring.position) < PORTAL_RADIUS
                })
            {
                let destination_schema = schemas
                    .iter()
                    .find(|schema| schema.id == endpoint.destination_maze)
                    .expect("validated portal destination must exist");
                let destination = destination_schema.portals[endpoint.destination_portal].clone();
                let destination_layout = layout_for(destination_schema);
                let destination_position =
                    destination_layout.world_position(portal_grid_cell(&destination));
                ring.maze_id = destination_schema.id;
                ring.position = destination_position;
                ring.origin = destination_position;
                ring.target = destination_position;
                ring.progress = 0.0;
                ring.duration = 0.0;
                ring.portal_cooldown = 1.2;
                ring.moving = false;
                ring.hopping = false;
                ring.growth = (ring.growth + 0.08).min(1.75);
                did_change = true;
                force_save = true;
            }
        }
    }
    world.rings = ring_states;
    world.rng_state = rng_state;
    world.dirty |= did_change;
    world.force_save |= force_save;

    for (entity, visual, mut transform) in &mut visuals {
        if let Some(ring) = world.rings.iter().find(|ring| ring.id == visual.id) {
            if ring.maze_id != world.active_maze {
                remove_visuals.push(entity);
                continue;
            }
            transform.translation = ring_render_position(ring);
            transform.rotation = Quat::from_rotation_x(0.28 + (ring.phase * 0.7).sin() * 0.12)
                * Quat::from_rotation_y(ring.phase * 0.35);
            transform.scale = Vec3::splat(ring_scale(ring));
        }
    }
    for entity in remove_visuals {
        commands.entity(entity).despawn();
    }

    let existing_ids = visuals
        .iter()
        .map(|(_, visual, _)| visual.id)
        .collect::<Vec<_>>();
    for ring in world
        .rings
        .iter()
        .filter(|ring| ring.maze_id == world.active_maze && !existing_ids.contains(&ring.id))
    {
        spawn_ring_visual(&mut commands, ring, &assets);
    }
}

fn walkable_neighbors(layout: &MazeLayout, position: Vec3) -> Vec<Vec3> {
    let (x, y) = layout.cell_at(position);
    [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)]
        .into_iter()
        .filter_map(|(dx, dy)| {
            let next_x = x + dx;
            let next_y = y + dy;
            (next_x >= 0 && next_y >= 0 && layout.is_walkable(next_x as usize, next_y as usize))
                .then(|| layout.world_position((next_x as usize, next_y as usize)))
        })
        .collect()
}

pub(crate) fn collect_traveling_ring(
    mut commands: Commands,
    mut world: ResMut<PortalWorld>,
    assets: Res<PortalAssets>,
    heroes: Query<&Transform, With<FireHero>>,
    visuals: Query<(Entity, &RingVisual)>,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let Some(index) = world.rings.iter().position(|ring| {
        ring.maze_id == world.active_maze
            && xz_distance(hero.translation, ring.position) < RING_CATCH_RADIUS
    }) else {
        return;
    };
    catch_ring(&mut commands, &mut world, &assets, &visuals, index);
}

/// Shared by walking and hook catches: score the ring, remove its visual, and spawn a replacement.
fn catch_ring(
    commands: &mut Commands,
    world: &mut PortalWorld,
    assets: &PortalAssets,
    visuals: &Query<(Entity, &RingVisual)>,
    index: usize,
) {
    let caught = world.rings.remove(index);
    world.caught_total = world.caught_total.saturating_add(1);
    world.caught_by_tier[caught.tier as usize - 1] =
        world.caught_by_tier[caught.tier as usize - 1].saturating_add(1);
    world.last_caught_tier = caught.tier;
    for (entity, visual) in visuals {
        if visual.id == caught.id {
            commands.entity(entity).despawn();
        }
    }
    let replacement = world.spawn_ring_record_in(caught.maze_id);
    world.rings.push(replacement.clone());
    spawn_ring_visual(commands, &replacement, assets);
    world.dirty = true;
    world.force_save = true;
}

// --- Hook -------------------------------------------------------------------------------------
// Plain-language summary: the cursor ray is intersected with a flat plane at ring height to get a
// world-space aim point. A left click starts the hook flying along an arc toward that point. Each
// frame the hook moves a little, and we test the *segment* it just travelled (previous → next
// position) against every ring in the active maze. Testing a segment instead of a single point
// means a fast hook cannot jump over a ring between two frames. A hit scores the ring exactly like
// walking into it, then the hook reels back to the fire. Hook state is never saved.

pub(crate) fn aim_portal_hook(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    world: Res<PortalWorld>,
    mut aim: ResMut<PortalHookAim>,
    mut marker: Single<(&mut Transform, &mut Visibility), With<PortalAimMarker>>,
) {
    let point = window
        .cursor_position()
        .and_then(|cursor| camera.0.viewport_to_world(camera.1, cursor).ok())
        .and_then(|ray| {
            ray.plane_intersection_point(
                Vec3::new(0.0, RING_HEIGHT, 0.0),
                InfinitePlane3d::new(Vec3::Y),
            )
        });
    let Some(point) = point else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    let mut point = world.layout.clamp_aim(point);
    point.y = RING_HEIGHT;
    aim.target = Some(point);
    marker.0.translation = point;
    *marker.1 = Visibility::Visible;
}

pub(crate) fn fire_portal_hook(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<PortalHookAim>,
    world: Res<PortalWorld>,
    heroes: Query<&Transform, (With<FireHero>, Without<PortalHook>)>,
    mut hooks: Query<(&mut Transform, &mut PortalHook, &mut Visibility), Without<FireHero>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(target), Some(hero), Ok((mut transform, mut hook, mut visibility))) =
        (aim.target, heroes.iter().next(), hooks.single_mut())
    else {
        return;
    };
    if hook.phase != HookPhase::Idle {
        return;
    }
    let start = hero.translation + Vec3::Y * HOOK_MUZZLE_HEIGHT;
    hook.phase = HookPhase::Flying;
    hook.maze_id = world.active_maze;
    hook.start = start;
    hook.target = target;
    hook.progress = 0.0;
    hook.duration = (start.distance(target) / HOOK_SPEED).max(0.08);
    transform.translation = start;
    *visibility = Visibility::Visible;
}

type PortalHookQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut PortalHook,
        &'static mut Visibility,
    ),
    Without<FireHero>,
>;

type PortalLinkQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Visibility,
        &'static PortalHookLink,
    ),
    (Without<FireHero>, Without<PortalHook>),
>;

#[derive(SystemParam)]
pub(crate) struct PortalHookParts<'w, 's> {
    hook: PortalHookQuery<'w, 's>,
    links: PortalLinkQuery<'w, 's>,
}

fn hook_arc_height(hook: &PortalHook) -> f32 {
    0.7 + hook.start.distance(hook.target) * 0.03
}

pub(crate) fn advance_portal_hook(
    mut commands: Commands,
    time: Res<Time>,
    mut world: ResMut<PortalWorld>,
    assets: Res<PortalAssets>,
    heroes: Query<&Transform, (With<FireHero>, Without<PortalHook>)>,
    mut parts: PortalHookParts,
    visuals: Query<(Entity, &RingVisual)>,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let Ok((mut transform, mut hook, mut visibility)) = parts.hook.single_mut() else {
        return;
    };
    let muzzle = hero.translation + Vec3::Y * HOOK_MUZZLE_HEIGHT;

    // Travelling through a portal replaces the maze, so a hook still in the air is cancelled.
    if hook.phase != HookPhase::Idle && hook.maze_id != world.active_maze {
        hook.phase = HookPhase::Idle;
    }

    if hook.phase == HookPhase::Idle {
        transform.translation = muzzle;
        *visibility = Visibility::Hidden;
        for (_, mut link_visibility, _) in &mut parts.links {
            *link_visibility = Visibility::Hidden;
        }
        return;
    }

    let step = time.delta_secs() / hook.duration.max(0.001);
    hook.progress = (hook.progress + step).min(1.0);
    if hook.phase == HookPhase::Flying {
        let previous = transform.translation;
        let next = arc_position(
            hook.start,
            hook.target,
            hook.progress,
            hook_arc_height(&hook),
        );
        transform.translation = next;

        let caught = world.rings.iter().position(|ring| {
            ring.maze_id == world.active_maze
                && distance_to_segment(ring_render_position(ring), previous, next)
                    <= HOOK_CATCH_RADIUS
        });
        if let Some(index) = caught {
            catch_ring(&mut commands, &mut world, &assets, &visuals, index);
        }
        if caught.is_some() || hook.progress >= 1.0 {
            hook.phase = HookPhase::Returning;
            hook.start = next;
            hook.target = muzzle;
            hook.progress = 0.0;
            hook.duration = (next.distance(muzzle) / HOOK_SPEED).max(0.08);
        }
    } else {
        // The fire may walk while the hook returns, so aim at its current muzzle position.
        hook.target = muzzle;
        transform.translation = hook.start.lerp(hook.target, hook.progress);
        if hook.progress >= 1.0 {
            hook.phase = HookPhase::Idle;
            *visibility = Visibility::Hidden;
        }
    }

    let end = transform.translation;
    let flying = hook.phase == HookPhase::Flying;
    for (mut link_transform, mut link_visibility, link) in &mut parts.links {
        let fraction = (link.index as f32 + 1.0) / (CHAIN_LINKS as f32 + 1.0);
        link_transform.translation = if flying {
            arc_position(
                hook.start,
                hook.target,
                hook.progress * fraction,
                hook_arc_height(&hook),
            )
        } else {
            muzzle.lerp(end, fraction)
        };
        link_transform.rotation = Quat::from_rotation_z(if link.index % 2 == 0 {
            std::f32::consts::FRAC_PI_2
        } else {
            0.0
        });
        *link_visibility = if hook.phase == HookPhase::Idle {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

pub(crate) fn enter_maze_portal(
    mut commands: Commands,
    mut world: ResMut<PortalWorld>,
    assets: Res<PortalAssets>,
    mut heroes: Query<&mut Transform, With<FireHero>>,
    scene_entities: Query<Entity, With<PortalSceneObject>>,
) {
    if world.player_portal_cooldown > 0.0 {
        return;
    }
    let Ok(mut hero) = heroes.single_mut() else {
        return;
    };
    let schema = world.active_schema().clone();
    let Some(endpoint) = schema.portals.iter().find(|portal| {
        let position = world.layout.world_position(portal_grid_cell(portal));
        xz_distance(hero.translation, position) <= PORTAL_RADIUS
    }) else {
        return;
    };
    let destination_id = endpoint.destination_maze;
    let destination_portal_index = endpoint.destination_portal;
    let destination_schema = world.schema(destination_id).clone();
    let destination_layout = layout_for(&destination_schema);
    let destination_endpoint = destination_schema.portals[destination_portal_index].clone();
    let destination_position =
        destination_layout.world_position(portal_grid_cell(&destination_endpoint));

    for entity in &scene_entities {
        commands.entity(entity).despawn();
    }
    world.active_maze = destination_id;
    world.layout = destination_layout;
    world.player_position = destination_position;
    world.player_portal_cooldown = 1.0;
    world.dirty = true;
    world.force_save = true;
    hero.translation = destination_position;
    world.balance_rings();
    spawn_active_maze(&mut commands, &world, &assets);
}

pub(crate) fn animate_portals(
    time: Res<Time>,
    mut portals: Query<(&PortalVisual, &mut Transform)>,
) {
    let seconds = time.elapsed_secs();
    for (portal, mut transform) in &mut portals {
        let pulse = 1.0 + (seconds * 2.0 + portal.phase).sin() * 0.045;
        transform.scale = Vec3::splat(pulse);
        transform.rotate_y(time.delta_secs() * if portal.index % 2 == 0 { 0.36 } else { -0.36 });
    }
}

pub(crate) fn update_portal_status(
    world: Res<PortalWorld>,
    mut status: Single<&mut Text, With<PortalMazeStatus>>,
) {
    let active_rings = world
        .rings
        .iter()
        .filter(|ring| ring.maze_id == world.active_maze)
        .collect::<Vec<_>>();
    let nearest = active_rings
        .iter()
        .min_by(|a, b| {
            xz_distance(a.position, world.player_position)
                .total_cmp(&xz_distance(b.position, world.player_position))
        })
        .map(|ring| format!("NEAREST LV {:02}", ring.tier))
        .unwrap_or_else(|| "NO RINGS HERE".to_string());
    let total_tier_72 = world.caught_by_tier[RING_TIERS - 1];
    status.0 = format!(
        "{}   ·   {} RINGS   ·   CAUGHT {}   ·   NEAR {}   ·   LAST {}   ·   LV72 {}",
        world.active_schema().name,
        active_rings.len(),
        world.caught_total,
        nearest,
        if world.last_caught_tier == 0 {
            "—".to_string()
        } else {
            format!("LV {:02}", world.last_caught_tier)
        },
        total_tier_72
    );
}

pub(crate) fn checkpoint_world(mut world: ResMut<PortalWorld>, time: Res<Time>) {
    world.save_timer += time.delta_secs();
    if !world.force_save && (!world.dirty || world.save_timer < SAVE_INTERVAL) {
        return;
    }
    let bytes = world.encode_binary();
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(parent) = world.save_path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            warn!("Could not create the Level 6 save folder: {error}");
            return;
        }
        if let Err(error) = fs::write(&world.save_path, &bytes) {
            warn!("Could not write Level 6 binary world state: {error}");
            return;
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            if storage
                .set_item(BROWSER_SAVE_KEY, &encode_base64(&bytes))
                .is_err()
            {
                warn!("Could not write Level 6 browser save data");
                return;
            }
        }
    }

    world.dirty = false;
    world.force_save = false;
    world.save_timer = 0.0;
}

pub(crate) fn remove_portal_maze_scene(
    mut commands: Commands,
    scene_entities: Query<Entity, With<PortalSceneObject>>,
    hook_gear: Query<Entity, With<PortalHookGear>>,
) {
    for entity in scene_entities.iter().chain(hook_gear.iter()) {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<PortalWorld>();
    commands.remove_resource::<PortalAssets>();
    commands.remove_resource::<PortalHookAim>();
}

fn decode_save(bytes: &[u8], schema_count: usize) -> Option<SavedWorld> {
    let mut reader = BinaryReader::new(bytes);
    if reader.take(4)? != SAVE_MAGIC || reader.u8()? != SAVE_VERSION {
        return None;
    }
    let active_maze = reader.u8()?;
    let next_ring_id = reader.u32()?;
    let caught_total = reader.u32()?;
    let last_caught_tier = reader.u8()?;
    let rng_state = reader.u64()?;
    let player_position = Vec3::new(reader.fixed_i16()?, 0.0, reader.fixed_i16()?);
    let ring_count = reader.u16()? as usize;
    if ring_count > MAX_SAVED_RINGS || active_maze as usize >= schema_count {
        return None;
    }
    let mut caught_by_tier = [0; RING_TIERS];
    for count in &mut caught_by_tier {
        *count = reader.u32()?;
    }
    let mut rings = Vec::with_capacity(ring_count);
    for _ in 0..ring_count {
        let id = reader.u32()?;
        let tier = reader.u8()?;
        let maze_id = reader.u8()?;
        let flags = reader.u8()?;
        let position = Vec3::new(reader.fixed_i16()?, 0.0, reader.fixed_i16()?);
        let origin = Vec3::new(reader.fixed_i16()?, 0.0, reader.fixed_i16()?);
        let target = Vec3::new(reader.fixed_i16()?, 0.0, reader.fixed_i16()?);
        let speed = reader.fixed_u16()?;
        let growth = reader.fixed_u16()?;
        let phase = reader.fixed_u16()?;
        let progress = reader.u16()? as f32 / u16::MAX as f32;
        let duration = reader.fixed_u16()?;
        let hop_height = reader.fixed_u16()?;
        let portal_cooldown = reader.fixed_u16()?;
        if !(1..=RING_TIERS as u8).contains(&tier)
            || maze_id as usize >= schema_count
            || !(0.1..=5.0).contains(&speed)
            || !(0.5..=1.75).contains(&growth)
        {
            return None;
        }
        rings.push(TravelRing {
            id,
            tier,
            maze_id,
            position,
            origin,
            target,
            speed,
            growth,
            phase,
            progress,
            duration,
            hop_height,
            portal_cooldown,
            moving: flags & 1 != 0,
            hopping: flags & 2 != 0,
        });
    }
    reader.is_done().then_some(SavedWorld {
        active_maze,
        next_ring_id,
        caught_total,
        caught_by_tier,
        last_caught_tier,
        player_position,
        rng_state,
        rings,
    })
}

fn load_saved_world(path: &std::path::Path, schema_count: usize) -> Option<SavedWorld> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        fs::read(path)
            .ok()
            .and_then(|bytes| decode_save(&bytes, schema_count))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        let storage = web_sys::window()?.local_storage().ok()??;
        let encoded = storage.get_item(BROWSER_SAVE_KEY).ok()??;
        decode_base64(&encoded).and_then(|bytes| decode_save(&bytes, schema_count))
    }
}

fn config_is_valid(config: &WorldConfig) -> bool {
    if config.maze_schemas.is_empty() || config.maze_schemas.len() > u8::MAX as usize {
        return false;
    }
    config.maze_schemas.iter().all(|schema| {
        !schema.portals.is_empty()
            && schema.portals.iter().all(|portal| {
                portal.cell[0] < schema.cells
                    && portal.cell[1] < schema.cells
                    && config.maze_schemas.iter().any(|target| {
                        target.id == portal.destination_maze
                            && portal.destination_portal < target.portals.len()
                    })
            })
    })
}

impl WorldConfig {
    fn fallback() -> Self {
        Self {
            ring_population: 4,
            maze_schemas: vec![
                MazeSchema {
                    id: 0,
                    name: "Cinder Hall".into(),
                    seed: 1481,
                    cells: 7,
                    extra_passages: 3,
                    portals: vec![
                        PortalEndpoint {
                            cell: [0, 0],
                            destination_maze: 1,
                            destination_portal: 0,
                        },
                        PortalEndpoint {
                            cell: [6, 6],
                            destination_maze: 1,
                            destination_portal: 1,
                        },
                    ],
                },
                MazeSchema {
                    id: 1,
                    name: "Azure Crossings".into(),
                    seed: 2917,
                    cells: 7,
                    extra_passages: 4,
                    portals: vec![
                        PortalEndpoint {
                            cell: [0, 0],
                            destination_maze: 0,
                            destination_portal: 0,
                        },
                        PortalEndpoint {
                            cell: [6, 6],
                            destination_maze: 0,
                            destination_portal: 1,
                        },
                    ],
                },
            ],
        }
    }
}

fn layout_for(schema: &MazeSchema) -> MazeLayout {
    MazeLayout::generate(schema.cells, schema.seed, Vec3::ZERO, schema.extra_passages)
}

fn portal_grid_cell(endpoint: &PortalEndpoint) -> (usize, usize) {
    (endpoint.cell[0] * 2 + 1, endpoint.cell[1] * 2 + 1)
}

fn roll_ring_tier(rng: &mut u64) -> u8 {
    let weight_total = (1..=RING_TIERS)
        .map(|tier| tier_weight(tier) as usize)
        .sum::<usize>();
    let mut draw = random_index(rng, weight_total);
    for tier in 1..=RING_TIERS {
        let weight = tier_weight(tier) as usize;
        if draw < weight {
            return tier as u8;
        }
        draw -= weight;
    }
    RING_TIERS as u8
}

fn tier_weight(tier: usize) -> u32 {
    (10_000 / (tier * tier) as u32).max(1)
}

fn ring_material_index(tier: u8) -> usize {
    match tier {
        1..=3 => 0,
        4..=12 => 1,
        13..=36 => 2,
        _ => 3,
    }
}

fn ring_scale(ring: &TravelRing) -> f32 {
    let tier_growth = 0.72 + (ring.tier as f32 - 1.0) / 71.0 * 0.58;
    tier_growth * ring.growth
}

fn ring_render_position(ring: &TravelRing) -> Vec3 {
    let jump = if ring.hopping {
        4.0 * ring.hop_height * ring.progress * (1.0 - ring.progress)
    } else {
        0.0
    };
    ring.position + Vec3::Y * (RING_HEIGHT + jump + (ring.phase * 2.2).sin() * 0.08)
}

fn xz_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn xz_segment_distance(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let point = Vec2::new(point.x, point.z);
    let start = Vec2::new(start.x, start.z);
    let segment = Vec2::new(end.x, end.z) - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

fn random_index(rng: &mut u64, upper: usize) -> usize {
    if upper == 0 {
        return 0;
    }
    next_random(rng) as usize % upper
}

fn random_unit(rng: &mut u64) -> f32 {
    (next_random(rng) >> 40) as f32 / ((1_u32 << 24) - 1) as f32
}

fn next_random(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}

#[cfg(target_arch = "wasm32")]
fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0b11) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[(((second & 0b1111) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(third & 0b11_1111) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(target_arch = "wasm32")]
fn decode_base64(encoded: &str) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut bits = 0_u8;
    for byte in encoded.bytes() {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            decoded.push((buffer >> bits) as u8);
            buffer &= (1_u32 << bits) - 1;
        }
    }
    Some(decoded)
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_fixed_i16(bytes: &mut Vec<u8>, value: f32) {
    let fixed = (value * 100.0)
        .round()
        .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    bytes.extend_from_slice(&fixed.to_le_bytes());
}

fn push_fixed_u16(bytes: &mut Vec<u8>, value: f32) {
    let fixed = (value * 100.0).round().clamp(0.0, u16::MAX as f32) as u16;
    push_u16(bytes, fixed);
}

struct BinaryReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> BinaryReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.cursor.checked_add(length)?;
        let part = self.bytes.get(self.cursor..end)?;
        self.cursor = end;
        Some(part)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn fixed_i16(&mut self) -> Option<f32> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?) as f32 / 100.0)
    }

    fn fixed_u16(&mut self) -> Option<f32> {
        Some(self.u16()? as f32 / 100.0)
    }

    fn is_done(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    /// The fallback worlds with no rings yet; the first maze is active and the fire is at the origin.
    pub(super) fn bare_portal_world(ring_population: usize) -> PortalWorld {
        let config = WorldConfig::fallback();
        let layout = layout_for(&config.maze_schemas[0]);
        PortalWorld {
            active_maze: config.maze_schemas[0].id,
            schemas: config.maze_schemas,
            layout,
            rings: Vec::new(),
            next_ring_id: 1,
            caught_total: 0,
            caught_by_tier: [0; RING_TIERS],
            last_caught_tier: 0,
            ring_population,
            player_position: Vec3::ZERO,
            player_portal_cooldown: 0.0,
            rng_state: 7,
            save_path: PathBuf::new(),
            save_timer: 0.0,
            dirty: false,
            force_save: false,
        }
    }

    /// A bare Bevy world holding one ring at `ring_at`, the fire at the origin, and the hook gear.
    fn hook_world(ring_at: impl Fn(&[Vec3]) -> Vec3) -> (World, Vec3) {
        let mut portal_world = bare_portal_world(2);
        let schema_id = portal_world.active_maze;
        let far_tiles = portal_world
            .layout
            .open_tiles()
            .into_iter()
            .filter(|tile| xz_distance(*tile, Vec3::ZERO) > 3.0)
            .collect::<Vec<_>>();
        let ring_position = ring_at(&far_tiles);

        let mut ring = portal_world.spawn_ring_record_in(schema_id);
        ring.position = ring_position;
        ring.origin = ring_position;
        ring.target = ring_position;
        ring.phase = 0.0;
        portal_world.rings.push(ring);

        let mut world = World::new();
        let assets = create_assets(
            &mut Assets::<Mesh>::default(),
            &mut Assets::<StandardMaterial>::default(),
        );
        world.insert_resource(assets);
        world.insert_resource(portal_world);
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.spawn((FireHero { speed: 3.0 }, Transform::default()));
        world
            .run_system_once(|mut commands: Commands, assets: Res<PortalAssets>| {
                spawn_portal_hook(&mut commands, &assets);
            })
            .unwrap();
        (world, ring_position)
    }

    fn click_at(world: &mut World, target: Vec3) {
        world.insert_resource(PortalHookAim {
            target: Some(Vec3::new(target.x, RING_HEIGHT, target.z)),
        });
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.run_system_once(fire_portal_hook).unwrap();
    }

    fn hook_phase(world: &mut World) -> HookPhase {
        world
            .query::<&PortalHook>()
            .single(world)
            .expect("one hook exists")
            .phase
    }

    /// Advances the hook in 20 ms frames until it has finished its whole out-and-back trip.
    fn run_until_idle(world: &mut World) {
        for _ in 0..400 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(20));
            world.run_system_once(advance_portal_hook).unwrap();
            if hook_phase(world) == HookPhase::Idle {
                return;
            }
        }
        panic!("the hook never returned to rest");
    }

    /// A point `distance` away from the ring, sideways to the line from the fire to the ring.
    fn beside(ring_position: Vec3, distance: f32) -> Vec3 {
        let forward = Vec3::new(ring_position.x, 0.0, ring_position.z).normalize();
        ring_position + Vec3::new(-forward.z, 0.0, forward.x) * distance
    }

    #[test]
    fn hook_catches_a_ring_aimed_slightly_off_center() {
        let (mut world, ring_position) = hook_world(|tiles| tiles[0]);
        click_at(&mut world, beside(ring_position, 0.5));
        assert!(hook_phase(&mut world) == HookPhase::Flying);

        run_until_idle(&mut world);

        let portal_world = world.resource::<PortalWorld>();
        assert_eq!(portal_world.caught_total, 1);
        assert_eq!(portal_world.rings.len(), 1, "a replacement ring appears");
    }

    #[test]
    fn hook_passing_well_beside_a_ring_misses_it() {
        let (mut world, ring_position) = hook_world(|tiles| tiles[0]);
        click_at(&mut world, beside(ring_position, 1.8));

        run_until_idle(&mut world);

        assert_eq!(world.resource::<PortalWorld>().caught_total, 0);
    }

    #[test]
    fn hook_aimed_at_empty_floor_catches_nothing() {
        let (mut world, ring_position) = hook_world(|tiles| tiles[0]);
        let empty_floor = world
            .resource::<PortalWorld>()
            .layout
            .open_tiles()
            .into_iter()
            .find(|tile| {
                xz_distance(*tile, Vec3::ZERO) > 3.0 && xz_distance(*tile, ring_position) > 4.0
            })
            .expect("the fallback maze has room for a second target");
        click_at(&mut world, empty_floor);

        run_until_idle(&mut world);

        assert_eq!(world.resource::<PortalWorld>().caught_total, 0);
    }

    #[test]
    fn a_second_click_is_ignored_while_the_hook_is_out() {
        let (mut world, ring_position) = hook_world(|tiles| tiles[0]);
        click_at(&mut world, ring_position);
        let first_target = world.query::<&PortalHook>().single(&world).unwrap().target;

        click_at(&mut world, Vec3::new(9.0, RING_HEIGHT, 9.0));

        let hook_target = world.query::<&PortalHook>().single(&world).unwrap().target;
        assert_eq!(hook_target, first_target);
    }

    fn count_by_maze(world: &PortalWorld) -> Vec<usize> {
        world
            .schemas
            .iter()
            .map(|schema| world.rings_in(schema.id))
            .collect()
    }

    #[test]
    fn every_maze_starts_with_its_own_rings() {
        let mut world = bare_portal_world(3);
        assert!(world.schemas.len() > 1, "the test needs several mazes");

        world.balance_rings();

        assert!(count_by_maze(&world).iter().all(|count| *count == 3));
        for schema in &world.schemas {
            let layout = layout_for(schema);
            assert!(
                world
                    .rings
                    .iter()
                    .filter(|ring| ring.maze_id == schema.id)
                    .all(|ring| layout.can_stand(ring.position)),
                "maze {} rings stand on open floor of their own layout",
                schema.id
            );
        }
    }

    #[test]
    fn a_save_with_most_rings_in_one_maze_fills_the_others() {
        let mut world = bare_portal_world(6);
        let first = world.active_maze;
        let second = world.schemas[1].id;
        for maze in [first, first, first, first, second] {
            let ring = world.spawn_ring_record_in(maze);
            world.rings.push(ring);
        }
        assert_eq!(world.rings_in(first), 4);

        world.balance_rings();

        assert!(count_by_maze(&world).iter().all(|count| *count == 6));
    }

    #[test]
    fn balancing_moves_spare_rings_instead_of_growing_the_total() {
        let mut world = bare_portal_world(2);
        let ids = world
            .schemas
            .iter()
            .map(|schema| schema.id)
            .collect::<Vec<_>>();
        // One maze is a ring short because a ring wandered through a portal into another maze.
        for maze in ids.iter().flat_map(|id| [*id, *id]) {
            let ring = world.spawn_ring_record_in(maze);
            world.rings.push(ring);
        }
        let wanderer = world
            .rings
            .iter()
            .position(|ring| ring.maze_id == ids[0])
            .unwrap();
        world.rings[wanderer].maze_id = ids[1];
        let total = world.rings.len();
        let moved_id = world.rings[wanderer].id;
        assert_eq!(world.rings_in(ids[1]), 3);

        world.balance_rings();

        assert_eq!(
            world.rings.len(),
            total,
            "no ring is created while one is spare"
        );
        assert!(count_by_maze(&world).iter().all(|count| *count == 2));
        let moved = world.rings.iter().find(|ring| ring.id == moved_id).unwrap();
        let layout = layout_for(world.schema(moved.maze_id));
        assert!(layout.can_stand(moved.position) && !moved.moving);
    }

    #[test]
    fn the_total_ring_count_never_passes_the_save_limit() {
        let mut world = bare_portal_world(MAX_SAVED_RINGS);
        world.balance_rings();
        assert_eq!(world.rings.len(), MAX_SAVED_RINGS);
    }
}
