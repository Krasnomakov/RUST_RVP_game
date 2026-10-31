use super::void_level::{FireHero, spawn_fire_hero};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::f32::consts::FRAC_PI_2;

const START_CELLS: usize = 7;
const MAX_CELLS: usize = 13;
const TILE_SIZE: f32 = 1.0;
const HERO_RADIUS: f32 = 0.27;
const RING_HEIGHT: f32 = 1.12;
const HOOK_SPEED: f32 = 15.0;
const HOOK_CATCH_RADIUS: f32 = 0.78;
const CHAIN_LINKS: usize = 24;

#[derive(Resource)]
pub(crate) struct EndlessMazeSession {
    pub(crate) rings_caught: u32,
    pub(crate) room: u32,
    layout: MazeLayout,
}

impl Default for EndlessMazeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl EndlessMazeSession {
    fn new() -> Self {
        Self {
            rings_caught: 0,
            room: 1,
            layout: MazeLayout::generate(START_CELLS, 1, Vec3::ZERO, 2),
        }
    }

    fn next_room(&mut self, destination: Vec3) {
        self.rings_caught += 1;
        self.room += 1;
        let cells = (START_CELLS + (self.rings_caught as usize / 2)).min(MAX_CELLS);
        let extra_passages = 2 + (self.rings_caught as usize / 2).min(30);
        let seed = 0xD1B5_4A32_D192_ED03_u64.wrapping_mul(self.room as u64);
        self.layout = MazeLayout::generate(cells, seed, destination, extra_passages);
    }

    pub(crate) fn camera_height(&self) -> f32 {
        13.0 + (self.layout.width as f32 * 0.12).min(4.0)
    }

    pub(crate) fn camera_depth(&self) -> f32 {
        11.0 + (self.layout.width as f32 * 0.10).min(3.5)
    }
}

#[derive(Resource)]
pub(crate) struct MazeAssets {
    floor: Handle<Mesh>,
    wall: Handle<Mesh>,
    ring: Handle<Mesh>,
    hook_segment: Handle<Mesh>,
    chain_link: Handle<Mesh>,
    marker: Handle<Mesh>,
    floor_materials: [Handle<StandardMaterial>; 2],
    wall_material: Handle<StandardMaterial>,
    ring_material: Handle<StandardMaterial>,
    hook_material: Handle<StandardMaterial>,
    chain_material: Handle<StandardMaterial>,
    marker_material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(crate) struct HookAim {
    target: Option<Vec3>,
}

#[derive(Component)]
pub(crate) struct EndlessMazeStatus;

#[derive(Component)]
pub(crate) struct MazeObject;

#[derive(Component)]
pub(crate) struct RingTarget {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
pub(crate) struct HookProjectile {
    flying: bool,
    returning: bool,
    start: Vec3,
    target: Vec3,
    progress: f32,
    duration: f32,
}

#[derive(Component)]
pub(crate) struct HookChain {
    index: usize,
}

#[derive(Component)]
pub(crate) struct AimMarker;

#[derive(Clone)]
pub(crate) struct MazeLayout {
    pub(crate) width: usize,
    walkable: Vec<bool>,
    start: (usize, usize),
    ring: (usize, usize),
    origin: Vec3,
}

impl MazeLayout {
    pub(crate) fn generate(cells: usize, seed: u64, origin: Vec3, extra_passages: usize) -> Self {
        let width = cells * 2 + 1;
        let mut layout = Self {
            width,
            walkable: vec![false; width * width],
            start: (0, 0),
            ring: (0, 0),
            origin,
        };
        let start_cell = (cells / 2, cells / 2);
        layout.start = (start_cell.0 * 2 + 1, start_cell.1 * 2 + 1);
        layout.set_walkable(layout.start.0, layout.start.1);

        let mut visited = vec![false; cells * cells];
        let start_index = start_cell.1 * cells + start_cell.0;
        visited[start_index] = true;
        let mut stack = vec![start_cell];
        let mut random = SmallRng(seed.max(1));

        // Iterative depth-first search carves one connected route through every maze cell.
        while let Some(&(cell_x, cell_y)) = stack.last() {
            let mut choices = Vec::with_capacity(4);
            for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                let next_x = cell_x as isize + dx;
                let next_y = cell_y as isize + dy;
                if next_x >= 0 && next_y >= 0 && next_x < cells as isize && next_y < cells as isize
                {
                    let index = next_y as usize * cells + next_x as usize;
                    if !visited[index] {
                        choices.push((next_x as usize, next_y as usize));
                    }
                }
            }

            if choices.is_empty() {
                stack.pop();
                continue;
            }

            let (next_x, next_y) = choices[random.range(choices.len())];
            visited[next_y * cells + next_x] = true;
            let here = (cell_x * 2 + 1, cell_y * 2 + 1);
            let next = (next_x * 2 + 1, next_y * 2 + 1);
            layout.set_walkable(next.0, next.1);
            layout.set_walkable((here.0 + next.0) / 2, (here.1 + next.1) / 2);
            stack.push((next_x, next_y));
        }

        // A few extra openings make intersections and alternate routes in later rooms.
        let mut closed_connectors = Vec::new();
        for y in (1..width - 1).step_by(2) {
            for x in (1..width - 1).step_by(2) {
                if x + 2 < width - 1 && !layout.is_walkable(x + 1, y) {
                    closed_connectors.push((x + 1, y));
                }
                if y + 2 < width - 1 && !layout.is_walkable(x, y + 1) {
                    closed_connectors.push((x, y + 1));
                }
            }
        }
        for _ in 0..extra_passages.min(closed_connectors.len()) {
            let index = random.range(closed_connectors.len());
            let (x, y) = closed_connectors.swap_remove(index);
            layout.set_walkable(x, y);
        }

        layout.ring = layout.choose_ring_cell();
        layout
    }

    fn set_walkable(&mut self, x: usize, y: usize) {
        self.walkable[y * self.width + x] = true;
    }

    pub(crate) fn is_walkable(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.width && self.walkable[y * self.width + x]
    }

    fn choose_ring_cell(&self) -> (usize, usize) {
        let mut distance = vec![u32::MAX; self.width * self.width];
        let mut queue = VecDeque::new();
        distance[self.start.1 * self.width + self.start.0] = 0;
        queue.push_back(self.start);
        while let Some((x, y)) = queue.pop_front() {
            let next_distance = distance[y * self.width + x] + 1;
            for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                let next_x = x as isize + dx;
                let next_y = y as isize + dy;
                if next_x < 0 || next_y < 0 {
                    continue;
                }
                let (next_x, next_y) = (next_x as usize, next_y as usize);
                let index = next_y * self.width + next_x;
                if self.is_walkable(next_x, next_y) && distance[index] == u32::MAX {
                    distance[index] = next_distance;
                    queue.push_back((next_x, next_y));
                }
            }
        }

        // Keep the first target within a useful hook range while choosing the most distant such tile.
        let mut chosen = self.start;
        let mut best_distance = 0;
        for y in (1..self.width - 1).step_by(2) {
            for x in (1..self.width - 1).step_by(2) {
                let steps = distance[y * self.width + x];
                if steps <= 12 && steps > best_distance {
                    chosen = (x, y);
                    best_distance = steps;
                }
            }
        }
        chosen
    }

    pub(crate) fn world_position(&self, cell: (usize, usize)) -> Vec3 {
        Vec3::new(
            self.origin.x + (cell.0 as f32 - self.start.0 as f32) * TILE_SIZE,
            0.0,
            self.origin.z + (cell.1 as f32 - self.start.1 as f32) * TILE_SIZE,
        )
    }

    pub(crate) fn cell_at(&self, position: Vec3) -> (isize, isize) {
        (
            (position.x - self.origin.x + self.start.0 as f32 + 0.5).floor() as isize,
            (position.z - self.origin.z + self.start.1 as f32 + 0.5).floor() as isize,
        )
    }

    pub(crate) fn can_stand(&self, position: Vec3) -> bool {
        [
            Vec2::ZERO,
            Vec2::new(HERO_RADIUS, 0.0),
            Vec2::new(-HERO_RADIUS, 0.0),
            Vec2::new(0.0, HERO_RADIUS),
            Vec2::new(0.0, -HERO_RADIUS),
        ]
        .into_iter()
        .all(|offset| {
            let (x, y) = self.cell_at(position + Vec3::new(offset.x, 0.0, offset.y));
            x >= 0 && y >= 0 && self.is_walkable(x as usize, y as usize)
        })
    }

    pub(crate) fn open_tiles(&self) -> Vec<Vec3> {
        let mut open = Vec::new();
        for y in 1..self.width - 1 {
            for x in 1..self.width - 1 {
                if self.is_walkable(x, y) {
                    open.push(self.world_position((x, y)));
                }
            }
        }
        open
    }

    pub(crate) fn clamp_aim(&self, mut point: Vec3) -> Vec3 {
        let min_x = self.origin.x - self.start.0 as f32 - 0.35;
        let max_x = self.origin.x + (self.width - 1 - self.start.0) as f32 + 0.35;
        let min_z = self.origin.z - self.start.1 as f32 - 0.35;
        let max_z = self.origin.z + (self.width - 1 - self.start.1) as f32 + 0.35;
        point.x = point.x.clamp(min_x, max_x);
        point.y = RING_HEIGHT;
        point.z = point.z.clamp(min_z, max_z);
        point
    }
}

struct SmallRng(u64);

impl SmallRng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper
    }
}

pub(crate) fn spawn_level_five_hero(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_fire_hero(&mut commands, &mut meshes, &mut materials);
}

pub(crate) fn start_endless_maze(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut session: ResMut<EndlessMazeSession>,
) {
    *session = EndlessMazeSession::new();
    let assets = MazeAssets {
        floor: meshes.add(Cuboid::new(0.98, 0.12, 0.98)),
        wall: meshes.add(Cuboid::new(1.0, 1.35, 1.0)),
        ring: meshes.add(Torus::new(0.32, 0.5)),
        hook_segment: meshes.add(Cylinder::new(0.035, 1.0)),
        chain_link: meshes.add(Torus::new(0.017, 0.052)),
        marker: meshes.add(Sphere::new(0.095)),
        floor_materials: [
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.075, 0.105, 0.16),
                perceptual_roughness: 0.92,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.105, 0.145, 0.21),
                perceptual_roughness: 0.92,
                ..default()
            }),
        ],
        wall_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.32, 0.43),
            metallic: 0.12,
            perceptual_roughness: 0.7,
            ..default()
        }),
        ring_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.23, 0.87, 1.0),
            emissive: LinearRgba::rgb(2.0, 8.0, 12.0),
            metallic: 0.72,
            perceptual_roughness: 0.18,
            ..default()
        }),
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
        marker_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.9, 1.0),
            emissive: LinearRgba::rgb(2.2, 8.0, 10.0),
            ..default()
        }),
    };
    spawn_maze(&mut commands, &session.layout, &assets);
    spawn_hook(&mut commands, &assets);
    commands.insert_resource(assets);
    commands.insert_resource(HookAim::default());
}

fn spawn_maze(commands: &mut Commands, layout: &MazeLayout, assets: &MazeAssets) {
    for y in 0..layout.width {
        for x in 0..layout.width {
            let position = layout.world_position((x, y));
            let tile_material = assets.floor_materials[(x + y) % 2].clone();
            commands.spawn((
                MazeObject,
                Mesh3d(assets.floor.clone()),
                MeshMaterial3d(tile_material),
                Transform::from_translation(position + Vec3::Y * -0.14),
            ));
            if !layout.is_walkable(x, y) {
                commands.spawn((
                    MazeObject,
                    Mesh3d(assets.wall.clone()),
                    MeshMaterial3d(assets.wall_material.clone()),
                    Transform::from_translation(position + Vec3::Y * 0.67),
                ));
            }
        }
    }

    let base = layout.world_position(layout.ring);
    commands.spawn((
        MazeObject,
        RingTarget { base, phase: 0.0 },
        Mesh3d(assets.ring.clone()),
        MeshMaterial3d(assets.ring_material.clone()),
        Transform {
            translation: base + Vec3::Y * RING_HEIGHT,
            rotation: Quat::from_rotation_x(0.32),
            ..default()
        },
    ));
}

fn spawn_hook(commands: &mut Commands, assets: &MazeAssets) {
    for index in 0..CHAIN_LINKS {
        commands.spawn((
            HookChain { index },
            Mesh3d(assets.chain_link.clone()),
            MeshMaterial3d(assets.chain_material.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }

    let hook = commands
        .spawn((
            HookProjectile {
                flying: false,
                returning: false,
                start: Vec3::ZERO,
                target: Vec3::ZERO,
                progress: 0.0,
                duration: 0.0,
            },
            Transform::from_xyz(0.0, 0.62, 0.0),
            Visibility::Hidden,
        ))
        .id();
    attach_hook_shape(commands, hook, &assets.hook_segment, &assets.hook_material);

    commands.spawn((
        AimMarker,
        Mesh3d(assets.marker.clone()),
        MeshMaterial3d(assets.marker_material.clone()),
        Transform::from_xyz(0.0, RING_HEIGHT, 0.0),
        Visibility::Hidden,
    ));
}

pub(crate) fn move_fire_in_maze(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    session: Res<EndlessMazeSession>,
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
        if session.layout.can_stand(candidate) {
            transform.translation.x = candidate.x;
        }
        candidate = transform.translation;
        candidate.z += delta.z * hero.speed;
        if session.layout.can_stand(candidate) {
            transform.translation.z = candidate.z;
        }
    }
}

pub(crate) fn aim_hook(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    session: Res<EndlessMazeSession>,
    mut aim: ResMut<HookAim>,
    mut marker: Single<(&mut Transform, &mut Visibility), With<AimMarker>>,
) {
    let Some(cursor) = window.cursor_position() else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    let Ok(ray) = camera.0.viewport_to_world(camera.1, cursor) else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    let Some(point) = ray.plane_intersection_point(
        Vec3::new(0.0, RING_HEIGHT, 0.0),
        InfinitePlane3d::new(Vec3::Y),
    ) else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    let point = session.layout.clamp_aim(point);
    aim.target = Some(point);
    marker.0.translation = point;
    *marker.1 = Visibility::Visible;
}

pub(crate) fn fire_hook(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<HookAim>,
    heroes: Query<&Transform, (With<FireHero>, Without<HookProjectile>)>,
    mut hooks: Query<(&mut Transform, &mut HookProjectile, &mut Visibility), Without<FireHero>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(target) = aim.target else {
        return;
    };
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    for (mut transform, mut hook, mut visibility) in &mut hooks {
        if hook.flying || hook.returning {
            continue;
        }
        let start = hero.translation + Vec3::Y * 0.62;
        let distance = start.distance(target);
        hook.start = start;
        hook.target = target;
        hook.progress = 0.0;
        hook.duration = (distance / HOOK_SPEED).max(0.08);
        hook.flying = true;
        hook.returning = false;
        transform.translation = start;
        *visibility = Visibility::Visible;
    }
}

pub(crate) fn animate_ring(time: Res<Time>, mut rings: Query<(&RingTarget, &mut Transform)>) {
    let seconds = time.elapsed_secs();
    for (ring, mut transform) in &mut rings {
        transform.translation =
            ring.base + Vec3::Y * (RING_HEIGHT + (seconds * 2.1 + ring.phase).sin() * 0.08);
        transform.rotate_y(time.delta_secs() * 0.8);
    }
}

type HookProjectileQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut HookProjectile,
        &'static mut Visibility,
    ),
    (With<HookProjectile>, Without<FireHero>),
>;

type HookChainQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Visibility,
        &'static HookChain,
    ),
    (With<HookChain>, Without<FireHero>, Without<HookProjectile>),
>;

type RingCatchQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static RingTarget),
    (
        Without<HookProjectile>,
        Without<HookChain>,
        Without<FireHero>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct HookWorld<'w, 's> {
    hooks: HookProjectileQuery<'w, 's>,
    chains: HookChainQuery<'w, 's>,
    rings: RingCatchQuery<'w, 's>,
    maze_objects: Query<'w, 's, Entity, With<MazeObject>>,
}

pub(crate) fn advance_hook(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<EndlessMazeSession>,
    assets: Res<MazeAssets>,
    mut heroes: Query<&mut Transform, (With<FireHero>, Without<HookProjectile>)>,
    mut hook_world: HookWorld,
) {
    let Ok(mut hero) = heroes.single_mut() else {
        return;
    };
    let Ok((mut transform, mut hook, mut visibility)) = hook_world.hooks.single_mut() else {
        return;
    };
    let delta_time = time.delta_secs();
    let muzzle = hero.translation + Vec3::Y * 0.62;

    if !hook.flying && !hook.returning {
        transform.translation = muzzle;
        *visibility = Visibility::Hidden;
        for (_, mut chain_visibility, _) in &mut hook_world.chains {
            *chain_visibility = Visibility::Hidden;
        }
        return;
    }

    if hook.flying {
        let previous = transform.translation;
        hook.progress = (hook.progress + delta_time / hook.duration.max(0.001)).min(1.0);
        let height = 2.2 + hook.start.distance(hook.target) * 0.07;
        let next = arc_position(hook.start, hook.target, hook.progress, height);
        transform.translation = next;

        let caught = hook_world
            .rings
            .iter()
            .find_map(|(entity, ring_transform, ring)| {
                (distance_to_segment(ring_transform.translation, previous, next)
                    <= HOOK_CATCH_RADIUS)
                    .then_some((entity, ring.base))
            });

        if let Some((_, base)) = caught {
            // The saved base position is the ring's floor location before the new room appears.
            let destination = Vec3::new(base.x, 0.0, base.z);
            hero.translation = destination;
            for entity in &hook_world.maze_objects {
                commands.entity(entity).despawn();
            }
            session.next_room(destination);
            spawn_maze(&mut commands, &session.layout, &assets);
            hook.flying = false;
            hook.returning = false;
            hook.progress = 0.0;
            *visibility = Visibility::Hidden;
            return;
        }

        if hook.progress >= 1.0 {
            hook.start = transform.translation;
            hook.target = muzzle;
            hook.progress = 0.0;
            hook.duration = (hook.start.distance(hook.target) / HOOK_SPEED).max(0.08);
            hook.flying = false;
            hook.returning = true;
        }
    } else if hook.returning {
        hook.progress = (hook.progress + delta_time / hook.duration.max(0.001)).min(1.0);
        transform.translation = hook.start.lerp(hook.target, hook.progress);
        if hook.progress >= 1.0 {
            hook.returning = false;
            *visibility = Visibility::Hidden;
        }
    }

    let end = transform.translation;
    for (mut link_transform, mut link_visibility, link) in &mut hook_world.chains {
        let fraction = (link.index as f32 + 1.0) / (CHAIN_LINKS as f32 + 1.0);
        let point = if hook.flying {
            arc_position(
                hook.start,
                hook.target,
                hook.progress * fraction,
                2.2 + hook.start.distance(hook.target) * 0.07,
            )
        } else {
            muzzle.lerp(end, fraction)
        };
        link_transform.translation = point;
        link_transform.rotation =
            Quat::from_rotation_z(if link.index % 2 == 0 { FRAC_PI_2 } else { 0.0 });
        *link_visibility = Visibility::Visible;
    }
}

pub(crate) fn update_status(
    session: Res<EndlessMazeSession>,
    mut status: Single<&mut Text, With<EndlessMazeStatus>>,
) {
    let size = session.layout.width;
    status.0 = format!(
        "MAZE {:03}   ·   RINGS {}   ·   GRID {} × {}   ·   LEFT CLICK TO THROW",
        session.room, session.rings_caught, size, size
    );
}

pub(crate) fn remove_endless_maze(
    mut commands: Commands,
    maze_objects: Query<Entity, With<MazeObject>>,
    hooks: Query<Entity, With<HookProjectile>>,
    chains: Query<Entity, With<HookChain>>,
    markers: Query<Entity, With<AimMarker>>,
) {
    for entity in maze_objects
        .iter()
        .chain(hooks.iter())
        .chain(chains.iter())
        .chain(markers.iter())
    {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<MazeAssets>();
    commands.remove_resource::<HookAim>();
}

/// Builds the curved metal hook from short stretched cylinders and parents them to `hook`.
pub(crate) fn attach_hook_shape(
    commands: &mut Commands,
    hook: Entity,
    segment: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
) {
    let points = [
        Vec3::new(0.0, 0.22, 0.0),
        Vec3::new(0.0, -0.015, 0.0),
        Vec3::new(0.07, -0.105, 0.0),
        Vec3::new(0.16, -0.13, 0.0),
        Vec3::new(0.235, -0.055, 0.0),
        Vec3::new(0.22, 0.035, 0.0),
        Vec3::new(0.145, 0.075, 0.0),
    ];
    commands.entity(hook).with_children(|parts| {
        for pair in points.windows(2) {
            let direction = pair[1] - pair[0];
            let length = direction.length();
            parts.spawn((
                Mesh3d(segment.clone()),
                MeshMaterial3d(material.clone()),
                Transform {
                    translation: (pair[0] + pair[1]) * 0.5,
                    rotation: Quat::from_rotation_arc(Vec3::Y, direction.normalize()),
                    scale: Vec3::new(1.0, length, 1.0),
                },
            ));
        }
    });
}

/// A point on a parabola from `start` to `target`; `height` is the extra lift at the midpoint.
pub(crate) fn arc_position(start: Vec3, target: Vec3, progress: f32, height: f32) -> Vec3 {
    let progress = progress.clamp(0.0, 1.0);
    start.lerp(target, progress) + Vec3::Y * (4.0 * height * progress * (1.0 - progress))
}

/// Shortest 3D distance from `point` to the line segment `start`..`end`.
pub(crate) fn distance_to_segment(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let progress = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * progress)
}
