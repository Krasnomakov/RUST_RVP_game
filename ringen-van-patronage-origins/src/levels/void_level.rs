use bevy::prelude::*;

const FLOOR_RADIUS: i32 = 4;
const FLOOR_EDGE: f32 = FLOOR_RADIUS as f32 + 0.48;

/// The controllable character. Its speed is measured in world units per second.
#[derive(Component)]
pub(crate) struct FireHero {
    pub(crate) speed: f32,
}

/// Marks entities that belong to The Void so the whole scene can be removed on exit.
#[derive(Component)]
pub(crate) struct VoidSceneObject;

#[derive(Component)]
pub(crate) struct FlameFlicker {
    phase: f32,
    sway: f32,
}

#[derive(Component)]
pub(crate) struct FireGlow;

pub(crate) fn spawn_void_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let tile_mesh = meshes.add(Cuboid::new(0.92, 0.14, 0.92));
    let tile_materials = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.17, 0.24),
            perceptual_roughness: 0.88,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.22, 0.30),
            perceptual_roughness: 0.88,
            ..default()
        }),
    ];

    // A small checkerboard gives the character a visible floor and a sense of scale.
    for x in -FLOOR_RADIUS..=FLOOR_RADIUS {
        for z in -FLOOR_RADIUS..=FLOOR_RADIUS {
            let material = tile_materials[((x + z).rem_euclid(2)) as usize].clone();
            commands.spawn((
                VoidSceneObject,
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(x as f32, -0.14, z as f32),
            ));
        }
    }

    spawn_fire_hero(&mut commands, &mut meshes, &mut materials);
}

/// Spawn only the shared fire character so later levels can provide their own floor.
pub(crate) fn spawn_fire_hero(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let outer_flame = meshes.add(Cone::new(0.36, 0.92));
    let middle_flame = meshes.add(Cone::new(0.27, 0.70));
    let inner_flame = meshes.add(Cone::new(0.16, 0.46));
    let outer_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.16, 0.015),
        emissive: LinearRgba::rgb(5.0, 0.22, 0.01),
        ..default()
    });
    let middle_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.40, 0.015),
        emissive: LinearRgba::rgb(7.0, 0.8, 0.015),
        ..default()
    });
    let inner_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.20),
        emissive: LinearRgba::rgb(9.0, 5.0, 0.35),
        ..default()
    });

    let hero = commands
        .spawn((
            VoidSceneObject,
            FireHero { speed: 3.0 },
            Transform::from_xyz(0.0, 0.0, 0.0),
            Visibility::default(),
        ))
        .id();

    commands.entity(hero).with_children(|parts| {
        parts.spawn((
            Mesh3d(outer_flame),
            MeshMaterial3d(outer_material),
            Transform::from_xyz(0.0, 0.48, 0.0),
            FlameFlicker {
                phase: 0.0,
                sway: 0.10,
            },
        ));
        parts.spawn((
            Mesh3d(middle_flame),
            MeshMaterial3d(middle_material),
            Transform::from_xyz(0.0, 0.38, 0.0),
            FlameFlicker {
                phase: 1.7,
                sway: 0.07,
            },
        ));
        parts.spawn((
            Mesh3d(inner_flame),
            MeshMaterial3d(inner_material),
            Transform::from_xyz(0.0, 0.27, 0.0),
            FlameFlicker {
                phase: 3.1,
                sway: 0.04,
            },
        ));
        parts.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.22, 0.035),
                intensity: 220.0,
                range: 3.2,
                ..default()
            },
            Transform::from_xyz(0.0, 0.42, 0.0),
            FireGlow,
        ));
    });
}

/// Convert held keys into a direction, then move at a frame-rate independent speed.
pub(crate) fn control_fire(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
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

    // Normalizing keeps diagonal movement from being faster than straight movement.
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }

    for (hero, mut transform) in &mut heroes {
        transform.translation += direction * hero.speed * time.delta_secs();
        transform.translation.x = transform.translation.x.clamp(-FLOOR_EDGE, FLOOR_EDGE);
        transform.translation.z = transform.translation.z.clamp(-FLOOR_EDGE, FLOOR_EDGE);
    }
}

/// Add a small, independent wobble to each flame layer and its point light.
pub(crate) fn animate_fire(
    time: Res<Time>,
    mut flames: Query<(&FlameFlicker, &mut Transform)>,
    mut glows: Query<&mut PointLight, With<FireGlow>>,
) {
    let seconds = time.elapsed_secs();
    for (flame, mut transform) in &mut flames {
        let flicker = (seconds * 13.0 + flame.phase).sin();
        transform.scale.y = 1.0 + flicker * 0.09;
        transform.rotation = Quat::from_rotation_z(flicker * flame.sway);
    }
    for mut glow in &mut glows {
        glow.intensity = 220.0 + (seconds * 11.0).sin().abs() * 100.0;
    }
}

pub(crate) fn remove_void_scene(
    mut commands: Commands,
    objects: Query<Entity, With<VoidSceneObject>>,
) {
    for entity in &objects {
        commands.entity(entity).despawn();
    }
}
