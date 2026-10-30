use bevy::prelude::*;
use std::f32::consts::TAU;

/// Shared environment settings. This resource defines how objects float in empty space.
#[derive(Resource)]
pub struct Space {
    pub float_height: f32,
    pub float_cycles_per_second: f32,
}

/// Per-ring properties attached to the torus entity.
#[derive(Component)]
pub struct Ring {
    resting_height: f32,
    spin_radians_per_second: f32,
}

pub fn spawn_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(0.78, 1.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.72, 0.95),
            metallic: 0.72,
            perceptual_roughness: 0.22,
            ..default()
        })),
        Transform::from_rotation(Quat::from_rotation_x(0.35)),
        Ring {
            resting_height: 0.0,
            spin_radians_per_second: 0.28,
        },
    ));
}

/// First control loop: evaluate a sine wave and add a small rotation every frame.
pub fn animate_ring(time: Res<Time>, space: Res<Space>, mut rings: Query<(&Ring, &mut Transform)>) {
    let seconds = time.elapsed_secs();
    let phase = seconds * TAU * space.float_cycles_per_second;

    for (ring, mut transform) in &mut rings {
        transform.translation.y = ring.resting_height + space.float_height * phase.sin();
        transform.rotate_y(ring.spin_radians_per_second * time.delta_secs());
    }
}
