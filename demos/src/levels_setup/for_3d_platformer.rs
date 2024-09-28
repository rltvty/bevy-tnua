// for_3d_platformer.rs

use bevy::{color::palettes::css, prelude::*};

#[cfg(feature = "avian3d")]
use avian3d::{prelude as avian, prelude::*};
#[cfg(feature = "rapier3d")]
use bevy_rapier3d::{prelude as rapier, prelude::*};
#[allow(unused_imports)]
use bevy_tnua::math::{AdjustPrecision, Vector3};
use bevy_tnua::TnuaGhostPlatform;

use crate::MovingPlatform;
use crate::levels_setup;

use super::{LevelObject, PositionPlayer};

#[cfg(feature = "avian3d")]
#[derive(PhysicsLayer)]
pub enum LayerNames {
    Player,
    FallThrough,
    PhaseThrough,
}

pub fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    level_settings: Res<levels_setup::level_switching::LevelSettings>, // Access LevelSettings resource
) {
    // let is_spherical = level_settings.is_spherical;
    let is_spherical = true;
    println!("Setting up level as spherical: {:?}", is_spherical);

    // Helper functions to adjust positions and transforms
    fn adjust_position(mut position: Vec3, is_spherical: bool) -> Vec3 {
        if is_spherical {
            let original_y = position.y;
    
            // Offset the position in the y direction
            position.y += 10.0; // Offset by +10 in y

            // Create a direction vector from the origin to the new position
            let direction = position.normalize();
    
            // Normalize and scale to project onto a sphere of radius 10
            position = direction * 10.0;
            
            // Move out by original_y units in the direction of the normalized vector
            position += direction * original_y;
        }
        position
    }

    fn adjust_transform(mut transform: Transform, is_spherical: bool) -> Transform {
        if is_spherical {
            // Adjust position
            let position = transform.translation;
            let adjusted_position = adjust_position(position, is_spherical);
            transform.translation = adjusted_position;

            // Adjust rotation to align "up" with the sphere's normal at that point
            let up = adjusted_position.normalize();
            let rotation = Quat::from_rotation_arc(Vec3::Y, up);
            transform.rotation = rotation * transform.rotation;
        }
        transform
    }

    fn adjust_positions(positions: &[Vector3], is_spherical: bool) -> Vec<Vector3> {
        positions
            .iter()
            .map(|&pos| {
                let vec3 = Vec3::new(pos.x, pos.y, pos.z);
                let adjusted_vec3 = adjust_position(vec3, is_spherical);
                Vector3::new(adjusted_vec3.x, adjusted_vec3.y, adjusted_vec3.z)
            })
            .collect()
    }

    // Adjust player position
    let player_position = adjust_position(Vec3::new(0.0, 0.0, 0.0), is_spherical);
    commands.spawn(PositionPlayer::from(player_position));

    let mut cmd = commands.spawn((LevelObject, Name::new("Floor")));
    if is_spherical {
        cmd.insert(PbrBundle {
            mesh: meshes.add(Sphere::new(10.0).mesh().ico(16).unwrap()),
            material: materials.add(Color::from(css::WHITE)),
            ..Default::default()
        });
        #[cfg(feature = "rapier3d")]
        cmd.insert(rapier::Collider::ball(10.0));
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::RigidBody::Static);
            cmd.insert(avian::Collider::sphere(10.0));
        }
    } else {
        cmd.insert(PbrBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(128.0, 128.0)),
            material: materials.add(Color::from(css::WHITE)),
            ..Default::default()
        });
        #[cfg(feature = "rapier3d")]
        cmd.insert(rapier::Collider::halfspace(Vec3::Y).unwrap());
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::RigidBody::Static);
            cmd.insert(avian::Collider::half_space(Vector3::Y));
        }
    }

    let obstacles_material = materials.add(Color::from(css::GRAY));
    for (name, [width, height, depth], mut transform) in [
        (
            "Moderate Slope",
            [10.0, 0.1, 2.0],
            Transform::from_xyz(7.0, 7.0, 0.0).with_rotation(Quat::from_rotation_z(0.6)),
        ),
        (
            "Steep Slope",
            [10.0, 0.1, 2.0],
            Transform::from_xyz(14.0, 14.0, 0.0).with_rotation(Quat::from_rotation_z(1.0)),
        ),
        (
            "Box to Step on",
            [4.0, 2.0, 2.0],
            Transform::from_xyz(-4.0, 1.0, 0.0),
        ),
        (
            "Floating Box",
            [6.0, 1.0, 2.0],
            Transform::from_xyz(-10.0, 4.0, 0.0),
        ),
        (
            "Box to Crawl Under",
            [6.0, 1.0, 2.0],
            Transform::from_xyz(0.0, 2.6, -5.0),
        ),
    ] {
        transform = adjust_transform(transform, is_spherical);

        let mut cmd = commands.spawn((LevelObject, Name::new(name)));
        cmd.insert(PbrBundle {
            mesh: meshes.add(Cuboid::new(width, height, depth)),
            material: obstacles_material.clone(),
            transform,
            ..Default::default()
        });
        #[cfg(feature = "rapier3d")]
        cmd.insert(rapier::Collider::cuboid(
            0.5 * width,
            0.5 * height,
            0.5 * depth,
        ));
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::RigidBody::Static);
            cmd.insert(avian::Collider::cuboid(
                width.adjust_precision(),
                height.adjust_precision(),
                depth.adjust_precision(),
            ));
        }
    }

    // Fall-through platforms
    let fall_through_obstacles_material = materials.add(Color::from(css::PINK).with_alpha(0.8));
    for (i, y) in [2.0, 4.5].into_iter().enumerate() {
        let position = Vec3::new(6.0, y, 10.0);
        let mut transform = Transform::from_translation(position);
        transform = adjust_transform(transform, is_spherical);

        let mut cmd = commands.spawn((LevelObject, Name::new(format!("Fall Through #{}", i + 1))));
        cmd.insert(PbrBundle {
            mesh: meshes.add(Cuboid::new(6.0, 0.5, 2.0)),
            material: fall_through_obstacles_material.clone(),
            transform,
            ..Default::default()
        });
        #[cfg(feature = "rapier3d")]
        {
            cmd.insert(rapier::Collider::cuboid(3.0, 0.25, 1.0));
            cmd.insert(SolverGroups {
                memberships: Group::empty(),
                filters: Group::empty(),
            });
        }
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::RigidBody::Static);
            cmd.insert(avian::Collider::cuboid(6.0, 0.5, 2.0));
            cmd.insert(CollisionLayers::new(
                [LayerNames::FallThrough],
                [LayerNames::FallThrough],
            ));
        }
        cmd.insert(TnuaGhostPlatform);
    }

    // Adjust and spawn other objects similarly
    let scene_positions = [
        ("Collision Groups", Vec3::new(10.0, 2.0, 1.0)),
        ("Sensor", Vec3::new(20.0, 2.0, 1.0)),
    ];

    for (name, position) in scene_positions.iter() {
        let mut transform = Transform::from_translation(*position);
        transform = adjust_transform(transform, is_spherical);

        let mut cmd = commands.spawn((
            LevelObject,
            Name::new(*name),
            SceneBundle {
                scene: asset_server.load(&format!(
                    "{}.glb#Scene0",
                    name.to_lowercase().replace(" ", "-")
                )),
                transform,
                ..Default::default()
            },
        ));

        if *name == "Collision Groups" {
            #[cfg(feature = "rapier3d")]
            {
                cmd.insert(rapier::Collider::cuboid(2.0, 1.0, 2.0));
                cmd.insert(CollisionGroups {
                    memberships: Group::GROUP_1,
                    filters: Group::GROUP_1,
                });
            }
            #[cfg(feature = "avian3d")]
            {
                cmd.insert(avian::RigidBody::Static);
                cmd.insert(avian::Collider::cuboid(4.0, 2.0, 4.0));
                cmd.insert(CollisionLayers::new(
                    [LayerNames::PhaseThrough],
                    [LayerNames::PhaseThrough],
                ));
            }
        } else if *name == "Sensor" {
            #[cfg(feature = "rapier3d")]
            {
                cmd.insert(rapier::Collider::cuboid(2.0, 1.0, 2.0));
                cmd.insert(rapier::Sensor);
            }
            #[cfg(feature = "avian3d")]
            {
                cmd.insert(avian::RigidBody::Static);
                cmd.insert(avian::Collider::cuboid(4.0, 2.0, 4.0));
                cmd.insert(avian::Sensor);
            }
        }
    }

    // Spawn moving platform
    {
        let mut transform = Transform::from_xyz(-4.0, 6.0, 0.0);
        transform = adjust_transform(transform, is_spherical);

        let mut cmd = commands.spawn((LevelObject, Name::new("Moving Platform")));
        cmd.insert(PbrBundle {
            mesh: meshes.add(Cuboid::new(4.0, 1.0, 4.0)),
            material: materials.add(Color::from(css::BLUE)),
            transform,
            ..Default::default()
        });
        #[cfg(feature = "rapier3d")]
        {
            cmd.insert(rapier::Collider::cuboid(2.0, 0.5, 2.0));
            cmd.insert(Velocity::default());
            cmd.insert(rapier::RigidBody::KinematicVelocityBased);
        }
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::Collider::cuboid(4.0, 1.0, 4.0));
            cmd.insert(avian::RigidBody::Kinematic);
        }
        let path_positions = &[
            Vector3::new(-4.0, 6.0, 0.0),
            Vector3::new(-8.0, 6.0, 0.0),
            Vector3::new(-8.0, 10.0, 0.0),
            Vector3::new(-8.0, 10.0, -4.0),
            Vector3::new(-4.0, 10.0, -4.0),
            Vector3::new(-4.0, 10.0, 0.0),
        ];
        let adjusted_path_positions = adjust_positions(path_positions, is_spherical);
        cmd.insert(MovingPlatform::new(4.0, &adjusted_path_positions));
    }

    // Spawn spinning platform
    {
        let mut transform = Transform::from_xyz(-2.0, 2.0, 10.0);
        transform = adjust_transform(transform, is_spherical);
        let direction = if is_spherical { transform.translation.normalize() } else {Vec3::Y};

        let mut cmd = commands.spawn((LevelObject, Name::new("Spinning Platform")));

        cmd.insert(PbrBundle {
            mesh: meshes.add(Cylinder::new(3.0, 1.0)),
            material: materials.add(Color::from(css::BLUE)),
            transform,
            ..Default::default()
        });
        #[cfg(feature = "rapier3d")]
        {
            cmd.insert(rapier::Collider::cylinder(0.5, 3.0));
            cmd.insert(Velocity::angular(direction));
            cmd.insert(rapier::RigidBody::KinematicVelocityBased);
        }
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::Collider::cylinder(3.0, 1.0));
            cmd.insert(AngularVelocity(direction));
            cmd.insert(avian::RigidBody::Kinematic);
        }
    }
}
