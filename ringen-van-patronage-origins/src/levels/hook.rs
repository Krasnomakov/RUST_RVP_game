use super::void_level::FireHero;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

const AIM_HEIGHT: f32 = 2.35;
const ARENA_EDGE: f32 = 4.48;
const HOOK_SPEED: f32 = 11.0;
const CATCH_RADIUS: f32 = 0.78;
const CHAIN_LINKS: usize = 20;

#[derive(Resource, Default)]
pub(crate) struct HookSession {
    rings_caught: u32,
    next_ring: u32,
    spawn_pending: bool,
}

#[derive(Resource, Default)]
pub(crate) struct HookAim {
    target: Option<Vec3>,
}

#[derive(Component)]
pub(crate) struct HookStatus;

#[derive(Component)]
pub(crate) struct HookSceneObject;

#[derive(Component)]
pub(crate) struct RingTarget {
    base: Vec3,
    phase: f32,
    caught: bool,
}

#[derive(Component)]
pub(crate) struct HookProjectile {
    phase: HookPhase,
    target: Vec3,
    captured_ring: Option<Entity>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum HookPhase {
    #[default]
    Idle,
    Flying,
    Returning,
}

#[derive(Component)]
pub(crate) struct HookChain {
    index: usize,
}

#[derive(Component)]
pub(crate) struct AimMarker;

type HeroTransformQuery<'w, 's> = Query<
    'w,
    's,
    &'static Transform,
    (
        With<FireHero>,
        Without<HookProjectile>,
        Without<HookChain>,
        Without<RingTarget>,
    ),
>;
type HookTransformQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut HookProjectile,
        &'static mut Visibility,
    ),
    (
        With<HookProjectile>,
        Without<FireHero>,
        Without<HookChain>,
        Without<RingTarget>,
    ),
>;
type ChainTransformQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Visibility,
        &'static HookChain,
    ),
    (
        With<HookChain>,
        Without<FireHero>,
        Without<HookProjectile>,
        Without<RingTarget>,
        Without<AimMarker>,
    ),
>;
type RingTransformQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Transform, &'static mut RingTarget),
    (
        With<RingTarget>,
        Without<FireHero>,
        Without<HookProjectile>,
        Without<HookChain>,
        Without<AimMarker>,
    ),
>;
type HookFiringQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut HookProjectile,
        &'static mut Visibility,
    ),
    (With<HookProjectile>, Without<FireHero>),
>;

pub(crate) fn start_hook_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut session: ResMut<HookSession>,
) {
    session.rings_caught = 0;
    session.next_ring = 0;
    session.spawn_pending = false;

    let ring_mesh = meshes.add(Torus::new(0.34, 0.53));
    let ring_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.57, 0.12),
        emissive: LinearRgba::rgb(5.2, 1.6, 0.18),
        metallic: 0.55,
        perceptual_roughness: 0.25,
        ..default()
    });
    spawn_ring_target(&mut commands, ring_mesh, ring_material, 0);

    let chain_mesh = meshes.add(Torus::new(0.017, 0.052));
    let chain_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.72, 0.78),
        metallic: 0.72,
        perceptual_roughness: 0.27,
        ..default()
    });
    for index in 0..CHAIN_LINKS {
        commands.spawn((
            HookSceneObject,
            HookChain { index },
            Mesh3d(chain_mesh.clone()),
            MeshMaterial3d(chain_material.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }

    let hook_root = commands
        .spawn((
            HookSceneObject,
            HookProjectile {
                phase: HookPhase::Idle,
                target: Vec3::ZERO,
                captured_ring: None,
            },
            Transform::from_xyz(0.0, 0.62, 0.0),
            Visibility::Hidden,
        ))
        .id();
    let segment_mesh = meshes.add(Cylinder::new(0.035, 1.0));
    let hook_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.68, 0.29),
        metallic: 0.78,
        perceptual_roughness: 0.22,
        ..default()
    });
    let points = [
        Vec3::new(0.0, 0.22, 0.0),
        Vec3::new(0.0, -0.015, 0.0),
        Vec3::new(0.07, -0.105, 0.0),
        Vec3::new(0.16, -0.13, 0.0),
        Vec3::new(0.235, -0.055, 0.0),
        Vec3::new(0.22, 0.035, 0.0),
        Vec3::new(0.145, 0.075, 0.0),
    ];
    commands.entity(hook_root).with_children(|parts| {
        for pair in points.windows(2) {
            let start = pair[0];
            let end = pair[1];
            let direction = end - start;
            let length = direction.length();
            parts.spawn((
                Mesh3d(segment_mesh.clone()),
                MeshMaterial3d(hook_material.clone()),
                Transform {
                    translation: (start + end) * 0.5,
                    rotation: Quat::from_rotation_arc(Vec3::Y, direction.normalize()),
                    scale: Vec3::new(1.0, length, 1.0),
                },
            ));
        }
    });

    let marker_mesh = meshes.add(Sphere::new(0.095));
    let marker_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.9, 1.0),
        emissive: LinearRgba::rgb(2.2, 8.0, 10.0),
        ..default()
    });
    commands.spawn((
        HookSceneObject,
        AimMarker,
        Mesh3d(marker_mesh),
        MeshMaterial3d(marker_material),
        Transform::from_xyz(0.0, AIM_HEIGHT, 0.0),
        Visibility::Hidden,
    ));
}

fn spawn_ring_target(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    sequence: u32,
) {
    let spots = [
        Vec3::new(2.4, 2.35, -1.7),
        Vec3::new(-2.6, 2.55, -2.2),
        Vec3::new(1.2, 2.25, 2.6),
        Vec3::new(-1.8, 2.5, 1.5),
        Vec3::new(3.1, 2.4, 0.8),
        Vec3::new(-3.0, 2.35, 0.5),
    ];
    let base = spots[sequence as usize % spots.len()];
    commands.spawn((
        HookSceneObject,
        RingTarget {
            base,
            phase: sequence as f32 * 1.31,
            caught: false,
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform {
            translation: base,
            rotation: Quat::from_rotation_x(0.36),
            ..default()
        },
    ));
}

pub(crate) fn aim_hook(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut aim: ResMut<HookAim>,
    mut marker: Single<(&mut Transform, &mut Visibility), With<AimMarker>>,
) {
    let Some(cursor) = window.cursor_position() else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    // A mouse position is 2D; the camera ray and horizontal plane turn it into a 3D aim point.
    let Ok(ray) = camera.0.viewport_to_world(camera.1, cursor) else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    let Some(mut point) = ray.plane_intersection_point(
        Vec3::new(0.0, AIM_HEIGHT, 0.0),
        InfinitePlane3d::new(Vec3::Y),
    ) else {
        aim.target = None;
        *marker.1 = Visibility::Hidden;
        return;
    };
    point.x = point.x.clamp(-ARENA_EDGE, ARENA_EDGE);
    point.y = AIM_HEIGHT;
    point.z = point.z.clamp(-ARENA_EDGE, ARENA_EDGE);
    aim.target = Some(point);
    marker.0.translation = point;
    *marker.1 = Visibility::Visible;
}

pub(crate) fn fire_hook(
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<HookAim>,
    heroes: Query<&Transform, (With<FireHero>, Without<HookProjectile>)>,
    mut hooks: HookFiringQuery,
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
        if hook.phase != HookPhase::Idle {
            continue;
        }
        transform.translation = hero.translation + Vec3::Y * 0.62;
        hook.target = target;
        hook.captured_ring = None;
        hook.phase = HookPhase::Flying;
        *visibility = Visibility::Visible;
    }
}

pub(crate) fn animate_ring_target(
    time: Res<Time>,
    mut rings: Query<(&RingTarget, &mut Transform)>,
) {
    let seconds = time.elapsed_secs();
    for (ring, mut transform) in &mut rings {
        if ring.caught {
            continue;
        }
        let phase = seconds * 0.68 + ring.phase;
        transform.translation = ring.base
            + Vec3::new(
                (phase * 0.47).sin() * 0.42,
                (phase * 1.13).sin() * 0.36,
                (phase * 0.39).cos() * 0.34,
            );
        transform.rotate_y(time.delta_secs() * 0.62);
    }
}

pub(crate) fn advance_hook(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<HookSession>,
    heroes: HeroTransformQuery,
    mut hooks: HookTransformQuery,
    mut links: ChainTransformQuery,
    mut rings: RingTransformQuery,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let muzzle = hero.translation + Vec3::Y * 0.62;
    let step = HOOK_SPEED * time.delta_secs();
    let Some((mut hook_transform, mut hook, mut hook_visibility)) = hooks.iter_mut().next() else {
        return;
    };

    // The hook moves through idle → flying → returning; only a returning hook can consume a ring.
    match hook.phase {
        HookPhase::Idle => {
            hook_transform.translation = muzzle;
            *hook_visibility = Visibility::Hidden;
        }
        HookPhase::Flying => {
            *hook_visibility = Visibility::Visible;
            let from = hook_transform.translation;
            let delta = hook.target - from;
            let distance = delta.length();
            let proposed = if distance <= step || distance <= f32::EPSILON {
                hook.target
            } else {
                from + delta / distance * step
            };

            let hit = rings.iter().next().and_then(|(entity, transform, ring)| {
                (!ring.caught
                    && distance_to_segment(transform.translation, from, proposed) <= CATCH_RADIUS)
                    .then_some((entity, transform.translation))
            });
            if let Some((ring_entity, ring_position)) = hit {
                hook_transform.translation = ring_position;
                hook.captured_ring = Some(ring_entity);
                hook.phase = HookPhase::Returning;
                if let Ok((_, mut ring_transform, mut ring)) = rings.get_mut(ring_entity) {
                    ring.caught = true;
                    ring_transform.translation = ring_position;
                }
            } else {
                hook_transform.translation = proposed;
                if distance <= step || distance <= f32::EPSILON {
                    hook.phase = HookPhase::Returning;
                }
            }
        }
        HookPhase::Returning => {
            *hook_visibility = Visibility::Visible;
            let from = hook_transform.translation;
            let delta = muzzle - from;
            let distance = delta.length();
            if distance <= step || distance <= 0.16 {
                hook_transform.translation = muzzle;
                if let Some(ring_entity) = hook.captured_ring.take() {
                    commands.entity(ring_entity).despawn();
                    session.rings_caught += 1;
                    session.next_ring += 1;
                    session.spawn_pending = true;
                }
                hook.phase = HookPhase::Idle;
                *hook_visibility = Visibility::Hidden;
            } else {
                hook_transform.translation += delta / distance * step;
            }
        }
    }

    if let Some(ring_entity) = hook.captured_ring
        && let Ok((_, mut ring_transform, mut ring)) = rings.get_mut(ring_entity)
    {
        ring.caught = true;
        ring_transform.translation = hook_transform.translation;
    }

    let active = hook.phase != HookPhase::Idle;
    let end = hook_transform.translation;
    let chain = end - muzzle;
    let chain_length = chain.length();
    if active && chain_length > f32::EPSILON {
        let direction = chain / chain_length;
        for (mut transform, mut visibility, link) in &mut links {
            let fraction = (link.index as f32 + 1.0) / (CHAIN_LINKS as f32 + 1.0);
            transform.translation = muzzle + chain * fraction;
            transform.rotation = Quat::from_rotation_arc(Vec3::Y, direction)
                * Quat::from_rotation_z(if link.index % 2 == 0 { FRAC_PI_2 } else { 0.0 });
            *visibility = Visibility::Visible;
        }
    } else {
        for (_, mut visibility, _) in &mut links {
            *visibility = Visibility::Hidden;
        }
    }
}

pub(crate) fn spawn_replacement_ring(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut session: ResMut<HookSession>,
) {
    if !session.spawn_pending {
        return;
    }
    session.spawn_pending = false;
    let mesh = meshes.add(Torus::new(0.34, 0.53));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.57, 0.12),
        emissive: LinearRgba::rgb(5.2, 1.6, 0.18),
        metallic: 0.55,
        perceptual_roughness: 0.25,
        ..default()
    });
    spawn_ring_target(&mut commands, mesh, material, session.next_ring);
}

pub(crate) fn update_status(
    session: Res<HookSession>,
    mut status: Single<&mut Text, With<HookStatus>>,
) {
    status.0 = format!(
        "RINGS CAUGHT: {}   ·   LEFT CLICK TO THROW",
        session.rings_caught
    );
}

pub(crate) fn remove_hook_scene(
    mut commands: Commands,
    objects: Query<Entity, With<HookSceneObject>>,
) {
    for entity in &objects {
        commands.entity(entity).despawn();
    }
}

fn distance_to_segment(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    // Test the whole frame's path so a fast hook cannot pass through a ring between updates.
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let progress = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * progress)
}
