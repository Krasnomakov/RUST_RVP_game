mod levels;
#[cfg(not(target_arch = "wasm32"))]
mod viewer_server;

use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    picking::hover::HoverMap,
    prelude::*,
    ui_widgets::{Activate, ActivateOnPress, Button as UiButton},
};
use levels::{
    endless_maze::{
        EndlessMazeSession, EndlessMazeStatus, advance_hook as advance_endless_hook,
        aim_hook as aim_endless_hook, animate_ring as animate_endless_ring,
        fire_hook as fire_endless_hook, move_fire_in_maze, remove_endless_maze,
        spawn_level_five_hero, start_endless_maze, update_status as update_endless_status,
    },
    hook::{
        HookAim, HookSession, HookStatus, advance_hook, aim_hook, animate_ring_target, fire_hook,
        remove_hook_scene, spawn_replacement_ring, start_hook_scene,
        update_status as update_hook_status,
    },
    maze::{
        MazeHero, MazeSession, MazeStatus, animate_maze, collect_maze_ring, move_maze_hero,
        remove_maze_scene, start_maze_scene, update_maze_status,
    },
    portal_maze::{
        PortalMazeStatus, PortalWorld, advance_portal_hook, advance_traveling_rings,
        aim_portal_hook, animate_portals, checkpoint_world, collect_traveling_ring,
        enter_maze_portal, fire_portal_hook, move_portal_hero, remove_portal_maze_scene,
        spawn_portal_hero, start_portal_maze_scene, update_portal_status,
    },
    ring::{Ring, Space, animate_ring, spawn_ring},
    void_level::{animate_fire, control_fire, remove_void_scene, spawn_void_scene},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, net::TcpStream, process::Command, thread, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if std::env::args().any(|argument| argument == "--viewer") {
        viewer_server::run(project_root());
        return;
    }

    run_game();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
fn browser_start() {
    run_game();
}

fn run_game() {
    let project_root = project_root();
    let catalog = LevelCatalog::load(&project_root);
    let maze_count = catalog.level_three.maze_count;

    let mut app = App::new();
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(DefaultPlugins);
    #[cfg(target_arch = "wasm32")]
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Ringen Van Patronage: Origins".into(),
            canvas: Some("#game-canvas".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }));

    app.init_state::<AppScreen>()
        .insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.035)))
        .insert_resource(Space {
            float_height: 0.45,
            float_cycles_per_second: 0.18,
        })
        .insert_resource(ProjectFiles { root: project_root })
        .insert_resource(catalog)
        .insert_resource(MazeSession::new(maze_count))
        .init_resource::<EndlessMazeSession>()
        .init_resource::<HookSession>()
        .init_resource::<HookAim>()
        .add_observer(on_menu_scroll)
        .add_systems(Startup, setup_cameras)
        .add_systems(OnEnter(AppScreen::Menu), build_menu)
        .add_systems(OnExit(AppScreen::Menu), clear_screen_ui)
        .add_systems(
            Update,
            (send_menu_scroll_events, scroll_menu_with_keyboard).run_if(in_state(AppScreen::Menu)),
        )
        .add_systems(
            OnEnter(AppScreen::RingLesson),
            (
                set_ring_camera,
                build_level_hud,
                spawn_ring_scene,
                record_level_opened,
            ),
        )
        .add_systems(
            OnExit(AppScreen::RingLesson),
            (clear_screen_ui, remove_ring),
        )
        .add_systems(
            OnEnter(AppScreen::VoidLesson),
            (
                set_void_camera,
                build_level_hud,
                spawn_void_scene,
                record_level_opened,
            ),
        )
        .add_systems(
            OnExit(AppScreen::VoidLesson),
            (clear_screen_ui, remove_void_scene),
        )
        .add_systems(
            OnEnter(AppScreen::MazeLesson),
            (
                set_maze_camera,
                build_level_hud,
                start_maze_scene,
                record_level_opened,
            ),
        )
        .add_systems(
            OnExit(AppScreen::MazeLesson),
            (clear_screen_ui, remove_maze_scene),
        )
        .add_systems(
            OnEnter(AppScreen::HookLesson),
            (
                set_void_camera,
                build_level_hud,
                spawn_void_scene,
                start_hook_scene,
                record_level_opened,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppScreen::HookLesson),
            (clear_screen_ui, remove_void_scene, remove_hook_scene).chain(),
        )
        .add_systems(
            OnEnter(AppScreen::EndlessMazeLesson),
            (
                set_endless_maze_camera,
                build_level_hud,
                spawn_level_five_hero,
                start_endless_maze,
                record_level_opened,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppScreen::EndlessMazeLesson),
            (clear_screen_ui, remove_endless_maze, remove_void_scene).chain(),
        )
        .add_systems(
            OnEnter(AppScreen::PortalMazeLesson),
            (
                set_portal_maze_camera,
                build_level_hud,
                spawn_portal_hero,
                start_portal_maze_scene,
                record_level_opened,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppScreen::PortalMazeLesson),
            (clear_screen_ui, remove_portal_maze_scene, remove_void_scene).chain(),
        )
        .add_systems(Update, animate_ring.run_if(in_state(AppScreen::RingLesson)))
        .add_systems(
            Update,
            (control_fire, animate_fire).run_if(in_state(AppScreen::VoidLesson)),
        )
        .add_systems(
            Update,
            (
                move_maze_hero,
                collect_maze_ring,
                animate_maze,
                update_maze_status,
            )
                .run_if(in_state(AppScreen::MazeLesson)),
        )
        .add_systems(
            Update,
            follow_maze_camera.run_if(in_state(AppScreen::MazeLesson)),
        )
        .add_systems(
            Update,
            (
                control_fire,
                animate_fire,
                aim_hook,
                fire_hook,
                animate_ring_target,
                advance_hook,
                spawn_replacement_ring,
                update_hook_status,
            )
                .chain()
                .run_if(in_state(AppScreen::HookLesson)),
        )
        .add_systems(
            Update,
            (
                move_fire_in_maze,
                animate_fire,
                aim_endless_hook,
                fire_endless_hook,
                animate_endless_ring,
                advance_endless_hook,
                update_endless_status,
            )
                .chain()
                .run_if(in_state(AppScreen::EndlessMazeLesson)),
        )
        .add_systems(
            Update,
            follow_endless_maze_camera.run_if(in_state(AppScreen::EndlessMazeLesson)),
        )
        .add_systems(
            Update,
            (
                move_portal_hero,
                animate_fire,
                aim_portal_hook,
                fire_portal_hook,
                advance_traveling_rings,
                advance_portal_hook,
                collect_traveling_ring,
                enter_maze_portal,
                animate_portals,
                update_portal_status,
                checkpoint_world,
            )
                .chain()
                .run_if(in_state(AppScreen::PortalMazeLesson)),
        )
        .add_systems(
            Update,
            follow_portal_maze_camera.run_if(in_state(AppScreen::PortalMazeLesson)),
        )
        .run();
}

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AppScreen {
    #[default]
    Menu,
    RingLesson,
    VoidLesson,
    MazeLesson,
    HookLesson,
    EndlessMazeLesson,
    PortalMazeLesson,
}

#[derive(Component)]
struct ScreenUi;

#[derive(Component)]
struct MenuScroller;

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct MenuScroll {
    entity: Entity,
    delta: Vec2,
}

#[derive(Component)]
struct GameCamera;

#[derive(Resource)]
pub(crate) struct ProjectFiles {
    pub(crate) root: PathBuf,
}

#[derive(Resource)]
struct LevelCatalog {
    level_one: LevelMetadata,
    level_two: LevelMetadata,
    level_three: LevelMetadata,
    level_four: LevelMetadata,
    level_five: LevelMetadata,
    level_six: LevelMetadata,
    load_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LevelMetadata {
    id: String,
    number: u32,
    title: String,
    subtitle: String,
    description: String,
    #[serde(default)]
    maze_count: u32,
}

impl LevelCatalog {
    #[cfg(not(target_arch = "wasm32"))]
    fn load(root: &Path) -> Self {
        let (level_one, error_one) = load_level(
            root,
            "data/levels/level-001.json",
            LevelMetadata::fallback_one(),
        );
        let (level_two, error_two) = load_level(
            root,
            "data/levels/level-002.json",
            LevelMetadata::fallback_two(),
        );
        let (level_three, error_three) = load_level(
            root,
            "data/levels/level-003.json",
            LevelMetadata::fallback_three(),
        );
        let (level_four, error_four) = load_level(
            root,
            "data/levels/level-004.json",
            LevelMetadata::fallback_four(),
        );
        let (level_five, error_five) = load_level(
            root,
            "data/levels/level-005.json",
            LevelMetadata::fallback_five(),
        );
        let (level_six, error_six) = load_level(
            root,
            "data/levels/level-006.json",
            LevelMetadata::fallback_six(),
        );
        Self {
            level_one,
            level_two,
            level_three,
            level_four,
            level_five,
            level_six,
            load_errors: [
                error_one,
                error_two,
                error_three,
                error_four,
                error_five,
                error_six,
            ]
            .into_iter()
            .flatten()
            .collect(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn load(_root: &Path) -> Self {
        fn read_embedded(text: &str, fallback: LevelMetadata) -> LevelMetadata {
            serde_json::from_str(text).unwrap_or(fallback)
        }

        Self {
            level_one: read_embedded(
                include_str!("../data/levels/level-001.json"),
                LevelMetadata::fallback_one(),
            ),
            level_two: read_embedded(
                include_str!("../data/levels/level-002.json"),
                LevelMetadata::fallback_two(),
            ),
            level_three: read_embedded(
                include_str!("../data/levels/level-003.json"),
                LevelMetadata::fallback_three(),
            ),
            level_four: read_embedded(
                include_str!("../data/levels/level-004.json"),
                LevelMetadata::fallback_four(),
            ),
            level_five: read_embedded(
                include_str!("../data/levels/level-005.json"),
                LevelMetadata::fallback_five(),
            ),
            level_six: read_embedded(
                include_str!("../data/levels/level-006.json"),
                LevelMetadata::fallback_six(),
            ),
            load_errors: Vec::new(),
        }
    }

    fn for_screen(&self, screen: AppScreen) -> &LevelMetadata {
        match screen {
            AppScreen::VoidLesson => &self.level_two,
            AppScreen::MazeLesson => &self.level_three,
            AppScreen::HookLesson => &self.level_four,
            AppScreen::EndlessMazeLesson => &self.level_five,
            AppScreen::PortalMazeLesson => &self.level_six,
            _ => &self.level_one,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_level(
    root: &Path,
    relative_file: &str,
    fallback: LevelMetadata,
) -> (LevelMetadata, Option<String>) {
    let full_path = root.join(relative_file);
    match fs::read_to_string(&full_path)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            serde_json::from_str::<LevelMetadata>(&text).map_err(|error| error.to_string())
        }) {
        Ok(level) => (level, None),
        Err(error) => (
            fallback,
            Some(format!("Could not read {relative_file}: {error}")),
        ),
    }
}

impl LevelMetadata {
    fn fallback_one() -> Self {
        Self {
            id: "level-001".into(),
            number: 1,
            title: "It all begins with a ring".into(),
            subtitle: "GEOMETRY · SPACE · MOTION".into(),
            description: "A single ring floats in a quiet 3D space.".into(),
            maze_count: 0,
        }
    }

    fn fallback_two() -> Self {
        Self {
            id: "level-002".into(),
            number: 2,
            title: "The Void".into(),
            subtitle: "KEYBOARD · MOVEMENT · TIME".into(),
            description: "Guide a small living flame across a tiled floor in empty space.".into(),
            maze_count: 0,
        }
    }

    fn fallback_three() -> Self {
        Self {
            id: "level-003".into(),
            number: 3,
            title: "The Labyrinth".into(),
            subtitle: "PROCEDURAL MAZES · SEARCH · COLLECTION".into(),
            description:
                "Guide the fire through 12 connected mazes and collect the ring in each one.".into(),
            maze_count: 12,
        }
    }

    fn fallback_four() -> Self {
        Self {
            id: "level-004".into(),
            number: 4,
            title: "The Hook".into(),
            subtitle: "MOUSE AIM · PROJECTILE · RETRIEVAL".into(),
            description: "Aim a chain hook at a floating ring, pull it back to the fire, and catch the next one.".into(),
            maze_count: 0,
        }
    }

    fn fallback_five() -> Self {
        Self {
            id: "level-005".into(),
            number: 5,
            title: "The Infinite Labyrinth".into(),
            subtitle: "ENDLESS GENERATION · HOOK TRAJECTORY · TELEPORTATION".into(),
            description: "Find a ring in each increasingly intricate maze. Throw the hook over walls; a catch teleports the fire to the ring and opens the next maze.".into(),
            maze_count: 0,
        }
    }

    fn fallback_six() -> Self {
        Self {
            id: "level-006".into(),
            number: 6,
            title: "The Ringways".into(),
            subtitle: "PORTAL NETWORK · 72 RING TIERS · BINARY WORLD STATE".into(),
            description: "Move through a network of maze worlds. Portals carry the fire and roaming rings between schemas; collect common and rare ring tiers.".into(),
            maze_count: 0,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize, Deserialize, Default)]
struct ProgressData {
    opened_levels: Vec<String>,
    last_opened_level: Option<String>,
}

fn project_root() -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var_os("RINGEN_PROJECT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
    }
    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::new()
    }
}

fn setup_cameras(mut commands: Commands) {
    // The lower-order 3D camera renders first; the 2D camera draws the menu over it.
    commands.spawn((
        GameCamera,
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        Transform::from_xyz(0.0, 1.8, 5.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 5_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.0, 4.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
}

fn send_menu_scroll_events(
    mut mouse_wheel_reader: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut commands: Commands,
) {
    for mouse_wheel in mouse_wheel_reader.read() {
        let mut delta = -Vec2::new(mouse_wheel.x, mouse_wheel.y);
        if mouse_wheel.unit == MouseScrollUnit::Line {
            delta *= 24.0;
        }

        for pointer_map in hover_map.values() {
            for entity in pointer_map.keys().copied() {
                commands.trigger(MenuScroll { entity, delta });
            }
        }
    }
}

fn on_menu_scroll(
    mut scroll: On<MenuScroll>,
    mut scrollers: Query<(&mut ScrollPosition, &Node, &ComputedNode), With<MenuScroller>>,
) {
    let Ok((mut position, node, computed)) = scrollers.get_mut(scroll.entity) else {
        return;
    };
    let max_offset = ((computed.content_size() - computed.size()) * computed.inverse_scale_factor)
        .max(Vec2::ZERO);

    if node.overflow.y == OverflowAxis::Scroll && scroll.delta.y != 0.0 {
        let at_limit = if scroll.delta.y > 0.0 {
            position.0.y >= max_offset.y
        } else {
            position.0.y <= 0.0
        };
        if !at_limit {
            position.0.y = (position.0.y + scroll.delta.y).clamp(0.0, max_offset.y);
            scroll.delta.y = 0.0;
        }
    }

    if scroll.delta == Vec2::ZERO {
        scroll.propagate(false);
    }
}

fn scroll_menu_with_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scrollers: Query<(&mut ScrollPosition, &ComputedNode), With<MenuScroller>>,
) {
    let Ok((mut position, computed)) = scrollers.single_mut() else {
        return;
    };
    let max_offset =
        (computed.content_size().y - computed.size().y).max(0.0) * computed.inverse_scale_factor;

    if keyboard.just_pressed(KeyCode::Home) {
        position.0.y = 0.0;
    } else if keyboard.just_pressed(KeyCode::End) {
        position.0.y = max_offset;
    } else if keyboard.just_pressed(KeyCode::PageDown) {
        position.0.y = (position.0.y + 320.0).min(max_offset);
    } else if keyboard.just_pressed(KeyCode::PageUp) {
        position.0.y = (position.0.y - 320.0).max(0.0);
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        position.0.y = (position.0.y + 72.0).min(max_offset);
    } else if keyboard.just_pressed(KeyCode::ArrowUp) {
        position.0.y = (position.0.y - 72.0).max(0.0);
    }
}

fn set_ring_camera(mut camera: Single<&mut Transform, With<GameCamera>>) {
    **camera = Transform::from_xyz(0.0, 1.8, 5.5).looking_at(Vec3::ZERO, Vec3::Y);
}

fn set_void_camera(mut camera: Single<&mut Transform, With<GameCamera>>) {
    **camera = Transform::from_xyz(0.0, 7.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y);
}

fn set_maze_camera(mut camera: Single<&mut Transform, With<GameCamera>>) {
    **camera = Transform::from_xyz(0.0, 14.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y);
}

fn set_endless_maze_camera(mut camera: Single<&mut Transform, With<GameCamera>>) {
    **camera = Transform::from_xyz(0.0, 14.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y);
}

fn set_portal_maze_camera(mut camera: Single<&mut Transform, With<GameCamera>>) {
    **camera = Transform::from_xyz(0.0, 14.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y);
}

fn follow_maze_camera(
    time: Res<Time>,
    mut camera: Single<&mut Transform, (With<GameCamera>, Without<MazeHero>)>,
    heroes: Query<&Transform, (With<MazeHero>, Without<GameCamera>)>,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let target = Transform::from_xyz(
        hero.translation.x,
        hero.translation.y + 13.0,
        hero.translation.z + 11.0,
    )
    .looking_at(hero.translation + Vec3::new(0.0, 0.0, 2.5), Vec3::Y);
    let blend = (time.delta_secs() * 4.5).clamp(0.0, 1.0);
    camera.translation = camera.translation.lerp(target.translation, blend);
    camera.rotation = camera.rotation.slerp(target.rotation, blend);
}

fn follow_endless_maze_camera(
    time: Res<Time>,
    session: Res<EndlessMazeSession>,
    mut last_room: Local<u32>,
    mut camera: Single<&mut Transform, (With<GameCamera>, Without<levels::void_level::FireHero>)>,
    heroes: Query<&Transform, (With<levels::void_level::FireHero>, Without<GameCamera>)>,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let target = Transform::from_xyz(
        hero.translation.x,
        hero.translation.y + session.camera_height(),
        hero.translation.z + session.camera_depth(),
    )
    .looking_at(hero.translation + Vec3::new(0.0, 0.0, 2.5), Vec3::Y);
    if *last_room != session.room {
        **camera = target;
        *last_room = session.room;
        return;
    }
    let blend = (time.delta_secs() * 4.5).clamp(0.0, 1.0);
    camera.translation = camera.translation.lerp(target.translation, blend);
    camera.rotation = camera.rotation.slerp(target.rotation, blend);
}

fn follow_portal_maze_camera(
    time: Res<Time>,
    world: Res<PortalWorld>,
    mut last_maze: Local<Option<u8>>,
    mut camera: Single<&mut Transform, (With<GameCamera>, Without<levels::void_level::FireHero>)>,
    heroes: Query<&Transform, (With<levels::void_level::FireHero>, Without<GameCamera>)>,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let target = Transform::from_xyz(
        hero.translation.x,
        hero.translation.y + world.camera_height(),
        hero.translation.z + world.camera_depth(),
    )
    .looking_at(hero.translation + Vec3::new(0.0, 0.0, 2.5), Vec3::Y);
    if *last_maze != Some(world.active_maze_id()) {
        **camera = target;
        *last_maze = Some(world.active_maze_id());
        return;
    }
    let blend = (time.delta_secs() * 4.5).clamp(0.0, 1.0);
    camera.translation = camera.translation.lerp(target.translation, blend);
    camera.rotation = camera.rotation.slerp(target.rotation, blend);
}

fn spawn_ring_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_ring(&mut commands, &mut meshes, &mut materials);
}

fn build_menu(mut commands: Commands, catalog: Res<LevelCatalog>, files: Res<ProjectFiles>) {
    let menu_hint = if cfg!(target_arch = "wasm32") {
        "Choose a level to play. The Rust source and learning notes are beside the game."
    } else {
        "Choose a level to play. Open the web studio for Rust code and learning materials."
    };
    commands
        .spawn((
            ScreenUi,
            MenuScroller,
            ScrollPosition::default(),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                row_gap: px(9),
                padding: UiRect::axes(px(18), px(12)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.012, 0.018, 0.045)),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Ringen Van Patronage: Origins"),
                TextFont {
                    font_size: FontSize::Px(32.0),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.9, 1.0)),
                TextLayout::justify(Justify::Center),
            ));
            root.spawn((
                Text::new(menu_hint),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.62, 0.71, 0.82)),
                TextLayout::justify(Justify::Center),
            ));
            root.spawn((
                Text::new("Scroll inside this menu or press Up/Down, Page Up, or Page Down to browse all levels."),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(0.45, 0.7, 0.82)),
                TextLayout::justify(Justify::Center),
            ));

            #[cfg(not(target_arch = "wasm32"))]
            spawn_learning_studio_button(root, "OPEN WEB LEARNING STUDIO  ↗", files.root.clone());
            #[cfg(target_arch = "wasm32")]
            let _ = &files.root;

            spawn_level_card(root, &catalog.level_one, AppScreen::RingLesson);
            spawn_level_card(root, &catalog.level_two, AppScreen::VoidLesson);
            spawn_level_card(root, &catalog.level_three, AppScreen::MazeLesson);
            spawn_level_card(root, &catalog.level_four, AppScreen::HookLesson);
            spawn_level_card(root, &catalog.level_five, AppScreen::EndlessMazeLesson);
            spawn_level_card(root, &catalog.level_six, AppScreen::PortalMazeLesson);

            for error in &catalog.load_errors {
                root.spawn((
                    Text::new(error.clone()),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.55, 0.4)),
                ));
            }
            root.spawn((
                Text::new("Game progress is stored locally in data/saves."),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.47, 0.55, 0.67)),
                TextLayout::justify(Justify::Center),
            ));
        });
}

fn spawn_level_card(
    parent: &mut ChildSpawnerCommands,
    level: &LevelMetadata,
    destination: AppScreen,
) {
    parent
        .spawn((
            Node {
                width: percent(82),
                max_width: px(760),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                padding: UiRect::all(px(14)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.055, 0.095)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(format!("LEVEL {:02}  ·  {}", level.number, level.subtitle)),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.35, 0.78, 0.94)),
            ));
            card.spawn((
                Text::new(level.title.clone()),
                TextFont {
                    font_size: FontSize::Px(21.0),
                    ..default()
                },
                TextColor(Color::srgb(0.91, 0.95, 1.0)),
            ));
            card.spawn((
                Text::new(level.description.clone()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.67, 0.75, 0.85)),
            ));
            card.spawn((
                UiButton,
                ActivateOnPress,
                Node {
                    width: percent(100),
                    min_height: px(42),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(px(4)),
                    padding: UiRect::axes(px(14), px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.35, 0.52)),
            ))
            .observe(
                move |_: On<Activate>, mut next: ResMut<NextState<AppScreen>>| {
                    next.set(destination);
                },
            )
            .with_children(|button| {
                button.spawn((
                    Text::new(format!("Open Level {:02}   →", level.number)),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_learning_studio_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    project_root: PathBuf,
) {
    parent
        .spawn((
            UiButton,
            ActivateOnPress,
            Node {
                min_width: px(190),
                min_height: px(40),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(12), px(7)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.3, 0.35)),
        ))
        .observe(move |_: On<Activate>| launch_learning_studio(project_root.clone()))
        .with_children(|button| {
            button.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn build_level_hud(
    mut commands: Commands,
    catalog: Res<LevelCatalog>,
    files: Res<ProjectFiles>,
    screen: Res<State<AppScreen>>,
) {
    let level = catalog.for_screen(*screen.get());
    let is_maze = *screen.get() == AppScreen::MazeLesson;
    let is_hook = *screen.get() == AppScreen::HookLesson;
    let is_endless_maze = *screen.get() == AppScreen::EndlessMazeLesson;
    let is_portal_maze = *screen.get() == AppScreen::PortalMazeLesson;
    let hint = match screen.get() {
        AppScreen::RingLesson => "Watch the ring float and rotate in space.",
        AppScreen::VoidLesson => "Move the flame with W/A/S/D or the arrow keys.",
        AppScreen::MazeLesson => "Move with W/A/S/D or the arrow keys. Find the ring to continue.",
        AppScreen::HookLesson => {
            "Move with W/A/S/D. Aim with the mouse and left-click to throw the hook."
        }
        AppScreen::EndlessMazeLesson => {
            "Move with W/A/S/D. Aim at a ring and left-click; the hook arcs over walls and teleports the fire on a catch."
        }
        AppScreen::PortalMazeLesson => {
            "Move with W/A/S/D. Aim with the mouse and left-click to throw the hook at a moving ring, or touch it. Walk into a glowing portal to travel between maze worlds."
        }
        AppScreen::Menu => "",
    };

    commands
        .spawn((
            ScreenUi,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(16)),
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(54),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: px(12),
                    padding: UiRect::axes(px(14), px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.88)),
            ))
            .with_children(|header| {
                header.spawn((
                    Text::new(format!("LEVEL {:02}  /  {}", level.number, level.title)),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.94, 1.0)),
                ));
                let project_root = files.root.clone();
                #[cfg(not(target_arch = "wasm32"))]
                spawn_learning_studio_button(header, "LEARNING STUDIO  ↗", project_root);
                #[cfg(target_arch = "wasm32")]
                let _ = project_root;
                header
                    .spawn((
                        UiButton,
                        ActivateOnPress,
                        Node {
                            min_width: px(105),
                            min_height: px(40),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(12), px(8)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.10, 0.18, 0.29)),
                    ))
                    .observe(|_: On<Activate>, mut next: ResMut<NextState<AppScreen>>| {
                        next.set(AppScreen::Menu);
                    })
                    .with_children(|button| {
                        button.spawn((
                            Text::new("←  Levels"),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });

            if is_maze {
                root.spawn((
                    MazeStatus,
                    Text::new(format!(
                        "MAZE 01 / {:02}   ·   RINGS 0 / {:02}",
                        level.maze_count, level.maze_count
                    )),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.37, 0.91, 0.96)),
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.84)),
                ));
            }

            if is_hook {
                root.spawn((
                    HookStatus,
                    Text::new("RINGS CAUGHT: 0   ·   LEFT CLICK TO THROW"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.72, 0.32)),
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.84)),
                ));
            }

            if is_endless_maze {
                root.spawn((
                    EndlessMazeStatus,
                    Text::new(
                        "MAZE 001   ·   RINGS 0   ·   GRID 15 × 15   ·   LEFT CLICK TO THROW",
                    ),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.37, 0.91, 0.96)),
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.84)),
                ));
            }

            if is_portal_maze {
                root.spawn((
                    PortalMazeStatus,
                    Text::new("CINDER HALL   ·   6 RINGS   ·   CAUGHT 0   ·   NEAR LV 01   ·   LAST —   ·   LV72 0"),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.48, 0.9, 1.0)),
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.84)),
                ));
            }

            root.spawn((
                Text::new(hint),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.87, 0.95)),
                Node {
                    padding: UiRect::axes(px(12), px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.68)),
            ));

            root.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
        });
}

#[cfg(not(target_arch = "wasm32"))]
fn launch_learning_studio(project_root: PathBuf) {
    thread::spawn(move || {
        let address = "127.0.0.1:4173";
        if TcpStream::connect(address).is_err()
            && let Err(error) = viewer_server::start(project_root)
            && TcpStream::connect(address).is_err()
        {
            warn!("Could not start the learning viewer: {error}");
            return;
        }

        for _ in 0..50 {
            if TcpStream::connect(address).is_ok() {
                open_in_default_app("http://127.0.0.1:4173/");
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        warn!("Learning viewer did not start at http://127.0.0.1:4173/");
    });
}

#[cfg(target_os = "macos")]
fn open_in_default_app(target: &str) {
    if let Err(error) = Command::new("open").arg(target).spawn() {
        warn!("Could not open the learning viewer: {error}");
    }
}

#[cfg(target_os = "windows")]
fn open_in_default_app(target: &str) {
    if let Err(error) = Command::new("cmd")
        .args(["/C", "start", "", target])
        .spawn()
    {
        warn!("Could not open the learning viewer: {error}");
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_default_app(target: &str) {
    if let Err(error) = Command::new("xdg-open").arg(target).spawn() {
        warn!("Could not open the learning viewer: {error}");
    }
}

fn clear_screen_ui(mut commands: Commands, screens: Query<Entity, With<ScreenUi>>) {
    for entity in &screens {
        commands.entity(entity).despawn();
    }
}

fn remove_ring(mut commands: Commands, rings: Query<Entity, With<Ring>>) {
    for entity in &rings {
        commands.entity(entity).despawn();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn record_level_opened(
    catalog: Res<LevelCatalog>,
    files: Res<ProjectFiles>,
    screen: Res<State<AppScreen>>,
) {
    let level = catalog.for_screen(*screen.get());
    let progress_path = files.root.join("data/saves/progress.json");
    if let Some(parent) = progress_path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        warn!(
            "Could not create progress folder {}: {error}",
            parent.display()
        );
        return;
    }
    let mut progress = fs::read_to_string(&progress_path)
        .ok()
        .and_then(|text| serde_json::from_str::<ProgressData>(&text).ok())
        .unwrap_or_default();

    if !progress.opened_levels.contains(&level.id) {
        progress.opened_levels.push(level.id.clone());
    }
    progress.last_opened_level = Some(level.id.clone());

    match serde_json::to_string_pretty(&progress) {
        Ok(json) => {
            if let Err(error) = fs::write(&progress_path, json) {
                warn!(
                    "Could not save level progress to {}: {error}",
                    progress_path.display()
                );
            }
        }
        Err(error) => warn!("Could not encode level progress: {error}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn record_level_opened() {}
