use bevy::{
    ecs::system::{EntityCommands, SystemParam},
    prelude::*,
};

#[cfg(feature = "avian3d")]
use avian3d::prelude as avian;
#[cfg(feature = "rapier3d")]
use bevy_rapier3d::prelude as rapier;

use bevy_tnua::math::{AsF32, Float, Vector3};

use crate::levels_setup::LevelObject;
use crate::levels_setup::level_switching::SwitchableLevels;

#[derive(SystemParam, Deref, DerefMut)]
pub struct LevelSetupHelper3d<'w, 's> {
    #[deref]
    pub commands: Commands<'w, 's>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    asset_server: Res<'w, AssetServer>,
    switchable_levels: Res<'w, SwitchableLevels>,
}

impl<'w, 's> LevelSetupHelper3d<'w, 's> {
    pub fn spawn_named(&mut self, name: impl ToString) -> EntityCommands {
        self.commands
            .spawn((LevelObject, Name::new(name.to_string())))
    }

    pub fn spawn_floor(&mut self, color: impl Into<Color>) -> EntityCommands {
        let is_spherical = self.is_spherical(); 

        let mesh = if is_spherical {
            self.meshes.add(Sphere::new(10.0).mesh().ico(16).unwrap())
        } else {
            self.meshes.add(Plane3d::default().mesh().size(128.0, 128.0))
        };
        
        let material = self.materials.add(color.into());
        let mut cmd = self.spawn_named("Floor");
        cmd.insert(PbrBundle {
            mesh,
            material,
            ..Default::default()
        });

        if is_spherical {
            #[cfg(feature = "rapier3d")]
            cmd.insert(rapier::Collider::ball(10.0));
            #[cfg(feature = "avian3d")]
            {
                cmd.insert(avian::RigidBody::Static);
                cmd.insert(avian::Collider::sphere(10.0));
            }
        } else {
            #[cfg(feature = "rapier3d")]
            cmd.insert(rapier::Collider::halfspace(Vec3::Y).unwrap());
            #[cfg(feature = "avian3d")]
            {
                cmd.insert(avian::RigidBody::Static);
                cmd.insert(avian::Collider::half_space(Vector3::Y));
            }
        }

        cmd
    }

    pub fn with_material<'a>(
        &'a mut self,
        material: impl Into<StandardMaterial>,
    ) -> LevelSetupHelper3dWithMaterial<'a, 'w, 's> {
        let material = self.materials.add(material);
        LevelSetupHelper3dWithMaterial {
            parent: self,
            material,
        }
    }

    pub fn with_color<'a>(
        &'a mut self,
        color: impl Into<Color>,
    ) -> LevelSetupHelper3dWithMaterial<'a, 'w, 's> {
        self.with_material(color.into())
    }

    pub fn spawn_scene_cuboid(
        &mut self,
        name: impl ToString,
        path: impl ToString,
        transform: Transform,
        #[allow(unused)] size: Vector3,
    ) -> EntityCommands {
        let transform = self.adjust_transform(transform);
        let scene = self.asset_server.load(path.to_string());
        let mut cmd = self.spawn_named(name);

        cmd.insert(SceneBundle {
            scene,
            transform,
            ..Default::default()
        });

        #[cfg(feature = "rapier3d")]
        cmd.insert(rapier::Collider::cuboid(
            0.5 * size.x.f32(),
            0.5 * size.y.f32(),
            0.5 * size.z.f32(),
        ));
        #[cfg(feature = "avian3d")]
        {
            cmd.insert(avian::RigidBody::Static);
            cmd.insert(avian::Collider::cuboid(size.x, size.y, size.z));
        }

        cmd
    }

    pub fn is_spherical(&self) -> bool {
        if let Some(switchable_level) = self.switchable_levels.levels.get(self.switchable_levels.current) {
            switchable_level.settings().is_spherical
        } else {
            false
        }
    }

    pub fn get_transform(&self, position: Vec3) -> Transform {
        let transform = Transform::from_translation(position);
        self.adjust_transform(transform)
    }

    pub fn adjust_transform(&self, mut transform: Transform) -> Transform {
        if self.is_spherical() {
            // Adjust position
            let position = transform.translation;
            let adjusted_position = self.adjust_position(position);
            transform.translation = adjusted_position;

            // Adjust rotation to align "up" with the sphere's normal at that point
            let up = adjusted_position.normalize();
            let rotation = Quat::from_rotation_arc(Vec3::Y, up);
            transform.rotation = rotation * transform.rotation;
        }
        transform
    }

    // Helper functions to adjust positions and transforms
    pub fn adjust_position(&self, mut position: Vec3) -> Vec3 {
        if self.is_spherical() {
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

    pub fn adjust_positions(&self, positions: &[Vector3]) -> Vec<Vector3> {
        positions
            .iter()
            .map(|&pos| {
                let vec3 = Vec3::new(pos.x, pos.y, pos.z);
                let adjusted_vec3 = self.adjust_position(vec3);
                Vector3::new(adjusted_vec3.x, adjusted_vec3.y, adjusted_vec3.z)
            })
            .collect()
    }
}

pub struct LevelSetupHelper3dWithMaterial<'a, 'w, 's> {
    parent: &'a mut LevelSetupHelper3d<'w, 's>,
    material: Handle<StandardMaterial>,
}

impl LevelSetupHelper3dWithMaterial<'_, '_, '_> {
    pub fn spawn_mesh_without_physics(
        &mut self,
        name: impl ToString,
        transform: Transform,
        mesh: impl Into<Mesh>,
    ) -> EntityCommands {
        let mesh = self.parent.meshes.add(mesh);
        let mut cmd = self.parent.spawn_named(name);
        cmd.insert(PbrBundle {
            mesh,
            material: self.material.clone(),
            transform,
            ..Default::default()
        });
        cmd
    }

    pub fn spawn_cuboid(
        &mut self,
        name: impl ToString,
        transform: Transform,
        size: Vector3,
    ) -> EntityCommands {
        let transform = self.parent.adjust_transform(transform);

        let mut cmd =
            self.spawn_mesh_without_physics(name, transform, Cuboid::from_size(size.f32()));

        cmd.insert((
            #[cfg(feature = "rapier3d")]
            rapier::Collider::cuboid(0.5 * size.x.f32(), 0.5 * size.y.f32(), 0.5 * size.z.f32()),
            #[cfg(feature = "avian3d")]
            (
                avian::RigidBody::Static,
                avian::Collider::cuboid(size.x, size.y, size.z),
            ),
        ));

        cmd
    }

    pub fn spawn_cylinder(
        &mut self,
        name: impl ToString,
        transform: Transform,
        radius: Float,
        half_height: Float,
    ) -> EntityCommands {
        let transform = self.parent.adjust_transform(transform);
        let mut cmd = self.spawn_mesh_without_physics(
            name,
            transform,
            Cylinder {
                radius: radius.f32(),
                half_height: half_height.f32(),
            },
        );

        cmd.insert((
            #[cfg(feature = "rapier3d")]
            rapier::Collider::cylinder(half_height, radius),
            #[cfg(feature = "avian3d")]
            (
                avian::RigidBody::Static,
                avian::Collider::cylinder(radius, 2.0 * half_height),
            ),
        ));

        cmd
    }
}

pub trait LevelSetupHelper3dEntityCommandsExtension {
    fn make_kinematic(&mut self) -> &mut Self;
    fn make_kinematic_with_linear_velocity(&mut self, velocity: Vector3) -> &mut Self;
    fn make_kinematic_with_angular_velocity(&mut self, angvel: Vector3) -> &mut Self;
    fn add_ball_collider(&mut self, radius: Float) -> &mut Self;
    fn make_sensor(&mut self) -> &mut Self;
}

impl LevelSetupHelper3dEntityCommandsExtension for EntityCommands<'_> {
    fn make_kinematic(&mut self) -> &mut Self {
        self.insert((
            #[cfg(feature = "avian3d")]
            avian::RigidBody::Kinematic,
            #[cfg(feature = "rapier3d")]
            (
                rapier::Velocity::default(),
                rapier::RigidBody::KinematicVelocityBased,
            ),
        ))
    }

    fn make_kinematic_with_linear_velocity(
        &mut self,
        #[allow(unused)] velocity: Vector3,
    ) -> &mut Self {
        self.insert((
            #[cfg(feature = "avian3d")]
            (avian::LinearVelocity(velocity), avian::RigidBody::Kinematic),
            #[cfg(feature = "rapier3d")]
            (
                rapier::Velocity::linear(velocity),
                rapier::RigidBody::KinematicVelocityBased,
            ),
        ))
    }

    fn make_kinematic_with_angular_velocity(
        &mut self,
        #[allow(unused)] angvel: Vector3,
    ) -> &mut Self {
        self.insert((
            #[cfg(feature = "avian3d")]
            (avian::AngularVelocity(angvel), avian::RigidBody::Kinematic),
            #[cfg(feature = "rapier3d")]
            (
                rapier::Velocity::angular(angvel),
                rapier::RigidBody::KinematicVelocityBased,
            ),
        ))
    }

    fn add_ball_collider(&mut self, #[allow(unused)] radius: Float) -> &mut Self {
        self.insert((
            #[cfg(feature = "avian3d")]
            avian::Collider::sphere(radius),
            #[cfg(feature = "rapier3d")]
            rapier::Collider::ball(radius),
        ))
    }

    fn make_sensor(&mut self) -> &mut Self {
        self.insert((
            #[cfg(feature = "avian3d")]
            avian::Sensor,
            #[cfg(feature = "rapier3d")]
            rapier::Sensor,
        ))
    }
}
