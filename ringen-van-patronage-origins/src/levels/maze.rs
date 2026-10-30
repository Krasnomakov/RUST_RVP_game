use bevy::prelude::*;
use std::collections::VecDeque;

const MAZE_CELLS: usize = 9;
const GRID_WIDTH: usize = MAZE_CELLS * 2 + 1;
const GRID_CENTER: f32 = (GRID_WIDTH as f32 - 1.0) / 2.0;
const TILE_SIZE: f32 = 1.0;
const HERO_COLLISION_RADIUS: f32 = 0.28;

#[derive(Resource)]
pub(crate) struct MazeSession {
    pub(crate) current: u32,
    pub(crate) total: u32,
    pub(crate) collected: u32,
    pub(crate) finished: bool,
    layout: MazeLayout,
}

impl MazeSession {
    pub(crate) fn new(total: u32) -> Self {
        let total = total.clamp(10, 20);
        Self {
            current: 1,
            total,
            collected: 0,
            finished: false,
            layout: MazeLayout::generate(1),
        }
    }

    fn reset(&mut self, total: u32) {
        *self = Self::new(total);
    }
}

#[derive(Component)]
pub(crate) struct MazeStatus;

#[derive(Component)]
pub(crate) struct MazeObject;

#[derive(Component)]
pub(crate) struct MazeHero {
    speed: f32,
}

#[derive(Component)]
pub(crate) struct MazeRing;

#[derive(Component)]
pub(crate) struct MazeFlame {
    phase: f32,
}

#[derive(Clone)]
struct MazeLayout {
    walkable: Vec<bool>,
    start: (usize, usize),
    goal: (usize, usize),
}

impl MazeLayout {
    fn generate(seed: u64) -> Self {
        let mut layout = Self {
            walkable: vec![false; GRID_WIDTH * GRID_WIDTH],
            start: (1, 1),
            goal: (1, 1),
        };
        let mut visited = [false; MAZE_CELLS * MAZE_CELLS];
        let mut stack = vec![(0usize, 0usize)];
        let mut random = SmallRng(seed.max(1));
        visited[0] = true;
        layout.set_walkable(1, 1);

        // Iterative depth-first search carves one connected corridor through every cell.
        while let Some(&(cell_x, cell_y)) = stack.last() {
            let mut choices = Vec::with_capacity(4);
            for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                let next_x = cell_x as isize + dx;
                let next_y = cell_y as isize + dy;
                if next_x >= 0
                    && next_y >= 0
                    && next_x < MAZE_CELLS as isize
                    && next_y < MAZE_CELLS as isize
                {
                    let index = next_y as usize * MAZE_CELLS + next_x as usize;
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
            visited[next_y * MAZE_CELLS + next_x] = true;
            let grid_x = cell_x * 2 + 1;
            let grid_y = cell_y * 2 + 1;
            let next_grid_x = next_x * 2 + 1;
            let next_grid_y = next_y * 2 + 1;
            layout.set_walkable(next_grid_x, next_grid_y);
            layout.set_walkable((grid_x + next_grid_x) / 2, (grid_y + next_grid_y) / 2);
            stack.push((next_x, next_y));
        }

        layout.goal = layout.farthest_from_start();
        layout
    }

    fn set_walkable(&mut self, x: usize, y: usize) {
        self.walkable[y * GRID_WIDTH + x] = true;
    }

    fn is_walkable(&self, x: usize, y: usize) -> bool {
        x < GRID_WIDTH && y < GRID_WIDTH && self.walkable[y * GRID_WIDTH + x]
    }

    fn farthest_from_start(&self) -> (usize, usize) {
        let mut distance = vec![u32::MAX; GRID_WIDTH * GRID_WIDTH];
        let mut queue = VecDeque::new();
        distance[GRID_WIDTH + 1] = 0;
        queue.push_back((1usize, 1usize));
        let mut farthest = (1usize, 1usize);

        while let Some((x, y)) = queue.pop_front() {
            let next_distance = distance[y * GRID_WIDTH + x] + 1;
            if next_distance > distance[farthest.1 * GRID_WIDTH + farthest.0] {
                farthest = (x, y);
            }
            for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                let next_x = x as isize + dx;
                let next_y = y as isize + dy;
                if next_x >= 0 && next_y >= 0 {
                    let (next_x, next_y) = (next_x as usize, next_y as usize);
                    let index = next_y * GRID_WIDTH + next_x;
                    if self.is_walkable(next_x, next_y) && distance[index] == u32::MAX {
                        distance[index] = next_distance;
                        queue.push_back((next_x, next_y));
                    }
                }
            }
        }
        farthest
    }

    fn world_position(&self, cell: (usize, usize)) -> Vec3 {
        Vec3::new(
            (cell.0 as f32 - GRID_CENTER) * TILE_SIZE,
            0.0,
            (cell.1 as f32 - GRID_CENTER) * TILE_SIZE,
        )
    }

    fn can_stand(&self, position: Vec3) -> bool {
        // Check the center and four sides of the fire's footprint so it does not clip through walls.
        [
            Vec2::ZERO,
            Vec2::new(HERO_COLLISION_RADIUS, 0.0),
            Vec2::new(-HERO_COLLISION_RADIUS, 0.0),
            Vec2::new(0.0, HERO_COLLISION_RADIUS),
            Vec2::new(0.0, -HERO_COLLISION_RADIUS),
        ]
        .into_iter()
        .all(|offset| {
            let grid_x = ((position.x + offset.x) / TILE_SIZE + GRID_CENTER + 0.5).floor() as isize;
            let grid_y = ((position.z + offset.y) / TILE_SIZE + GRID_CENTER + 0.5).floor() as isize;
            grid_x >= 0 && grid_y >= 0 && self.is_walkable(grid_x as usize, grid_y as usize)
        })
    }
}

struct SmallRng(u64);

impl SmallRng {
    fn next(&mut self) -> u64 {
        // A compact deterministic xorshift generator is enough for lesson mazes.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper
    }
}

pub(crate) fn start_maze_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut session: ResMut<MazeSession>,
) {
    let total = session.total;
    session.reset(total);
    spawn_maze_entities(&mut commands, &mut meshes, &mut materials, &session.layout);
}

fn spawn_maze_entities(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    layout: &MazeLayout,
) {
    let floor_mesh = meshes.add(Cuboid::new(0.98, 0.12, 0.98));
    let wall_mesh = meshes.add(Cuboid::new(1.0, 1.35, 1.0));
    let floor_materials = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.12, 0.18),
            perceptual_roughness: 0.92,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.16, 0.23),
            perceptual_roughness: 0.92,
            ..default()
        }),
    ];
    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.31, 0.42),
        metallic: 0.18,
        perceptual_roughness: 0.68,
        ..default()
    });

    // Every grid cell has a floor tile; closed cells also receive a raised wall.
    for y in 0..GRID_WIDTH {
        for x in 0..GRID_WIDTH {
            let position = Vec3::new(x as f32 - GRID_CENTER, -0.14, y as f32 - GRID_CENTER);
            let floor_color = floor_materials[(x + y) % 2].clone();
            commands.spawn((
                MazeObject,
                Mesh3d(floor_mesh.clone()),
                MeshMaterial3d(floor_color),
                Transform::from_translation(position),
            ));
            if !layout.is_walkable(x, y) {
                commands.spawn((
                    MazeObject,
                    Mesh3d(wall_mesh.clone()),
                    MeshMaterial3d(wall_material.clone()),
                    Transform::from_translation(position + Vec3::Y * 0.67),
                ));
            }
        }
    }

    let flame_outer = meshes.add(Cone::new(0.34, 0.86));
    let flame_inner = meshes.add(Cone::new(0.19, 0.58));
    let fire_outer_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.16, 0.015),
        emissive: LinearRgba::rgb(5.0, 0.22, 0.01),
        ..default()
    });
    let fire_inner_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.72, 0.06),
        emissive: LinearRgba::rgb(8.0, 3.2, 0.08),
        ..default()
    });
    let hero_position = layout.world_position(layout.start);
    let hero = commands
        .spawn((
            MazeObject,
            MazeHero { speed: 3.2 },
            Transform::from_translation(hero_position),
            Visibility::default(),
        ))
        .id();
    commands.entity(hero).with_children(|parts| {
        parts.spawn((
            Mesh3d(flame_outer),
            MeshMaterial3d(fire_outer_material),
            Transform::from_xyz(0.0, 0.45, 0.0),
            MazeFlame { phase: 0.0 },
        ));
        parts.spawn((
            Mesh3d(flame_inner),
            MeshMaterial3d(fire_inner_material),
            Transform::from_xyz(0.0, 0.32, 0.0),
            MazeFlame { phase: 2.4 },
        ));
        parts.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.24, 0.035),
                intensity: 250.0,
                range: 3.5,
                ..default()
            },
            Transform::from_xyz(0.0, 0.4, 0.0),
        ));
    });

    let ring_position = layout.world_position(layout.goal) + Vec3::Y * 0.72;
    let ring_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.88, 1.0),
        metallic: 0.7,
        perceptual_roughness: 0.18,
        emissive: LinearRgba::rgb(2.0, 8.0, 12.0),
        ..default()
    });
    commands.spawn((
        MazeObject,
        MazeRing,
        Mesh3d(meshes.add(Torus::new(0.28, 0.46))),
        MeshMaterial3d(ring_material),
        Transform::from_translation(ring_position),
    ));
}

/// Read held keys, normalize the direction, and reject steps into wall cells.
pub(crate) fn move_maze_hero(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    session: Res<MazeSession>,
    mut heroes: Query<(&MazeHero, &mut Transform)>,
) {
    if session.finished {
        return;
    }
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

pub(crate) fn collect_maze_ring(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut session: ResMut<MazeSession>,
    heroes: Query<&Transform, With<MazeHero>>,
    rings: Query<(Entity, &Transform), With<MazeRing>>,
    objects: Query<Entity, With<MazeObject>>,
) {
    if session.finished {
        return;
    }
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let Some((ring_entity, ring)) = rings.iter().next() else {
        return;
    };
    let distance = Vec2::new(
        hero.translation.x - ring.translation.x,
        hero.translation.z - ring.translation.z,
    )
    .length();
    if distance > 0.62 {
        return;
    }

    session.collected += 1;
    if session.current >= session.total {
        commands.entity(ring_entity).despawn();
        session.finished = true;
        return;
    }

    for entity in &objects {
        commands.entity(entity).despawn();
    }
    session.current += 1;
    session.layout = MazeLayout::generate(0x9E37_79B9_u64.wrapping_mul(session.current as u64));
    spawn_maze_entities(&mut commands, &mut meshes, &mut materials, &session.layout);
}

pub(crate) fn animate_maze(
    time: Res<Time>,
    session: Res<MazeSession>,
    mut rings: Query<&mut Transform, (With<MazeRing>, Without<MazeFlame>)>,
    mut flames: Query<(&MazeFlame, &mut Transform), Without<MazeRing>>,
) {
    if session.finished {
        return;
    }
    for mut transform in &mut rings {
        transform.rotate_y(time.delta_secs() * 1.2);
        transform.translation.y = 0.72 + (time.elapsed_secs() * 2.5).sin() * 0.08;
    }
    for (flame, mut transform) in &mut flames {
        let flicker = (time.elapsed_secs() * 14.0 + flame.phase).sin();
        transform.scale.y = 1.0 + flicker * 0.08;
        transform.rotation = Quat::from_rotation_z(flicker * 0.07);
    }
}

pub(crate) fn update_maze_status(
    session: Res<MazeSession>,
    mut status: Single<&mut Text, With<MazeStatus>>,
) {
    status.0 = if session.finished {
        format!("ALL {} RINGS COLLECTED  ·  MAZE COMPLETE", session.total)
    } else {
        format!(
            "MAZE {:02} / {:02}   ·   RINGS {} / {:02}",
            session.current, session.total, session.collected, session.total
        )
    };
}

pub(crate) fn remove_maze_scene(mut commands: Commands, objects: Query<Entity, With<MazeObject>>) {
    for entity in &objects {
        commands.entity(entity).despawn();
    }
}
