//! # bevy_rapier3d Integration for bevy-tnua
//!
//! In addition to the instruction in bevy-tnua's documentation:
//!
//! * Add [`TnuaRapier3dPlugin`] to the Bevy app.
//! * Add [`TnuaRapier3dIOBundle`] to each character entity controlled by Tnua.
//! * Optionally: Add [`TnuaRapier3dSensorShape`] to the sensor entities. This means the entity of
//!   the characters controlled by Tnua, but also other things like the entity generated by
//!   `TnuaCrouchEnforcer`, that can be affected with a closure.
use bevy::ecs::schedule::{InternedScheduleLabel, ScheduleLabel};
use bevy::prelude::*;
use bevy::utils::HashSet;
use bevy_rapier3d::prelude::*;
use bevy_rapier3d::rapier;
use bevy_rapier3d::rapier::prelude::InteractionGroups;

use bevy_tnua_physics_integration_layer::data_for_backends::TnuaGhostPlatform;
use bevy_tnua_physics_integration_layer::data_for_backends::TnuaGhostSensor;
use bevy_tnua_physics_integration_layer::data_for_backends::TnuaToggle;
use bevy_tnua_physics_integration_layer::data_for_backends::{
    TnuaMotor, TnuaProximitySensor, TnuaProximitySensorOutput, TnuaRigidBodyTracker,
};
use bevy_tnua_physics_integration_layer::subservient_sensors::TnuaSubservientSensor;
use bevy_tnua_physics_integration_layer::TnuaPipelineStages;
use bevy_tnua_physics_integration_layer::TnuaSystemSet;

/// Add this plugin to use bevy_rapier3d as a physics backend.
///
/// This plugin should be used in addition to `TnuaControllerPlugin`.
pub struct TnuaRapier3dPlugin {
    schedule: InternedScheduleLabel,
}

impl TnuaRapier3dPlugin {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self {
            schedule: schedule.intern(),
        }
    }
}

impl Default for TnuaRapier3dPlugin {
    fn default() -> Self {
        Self::new(Update)
    }
}

impl Plugin for TnuaRapier3dPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            self.schedule,
            TnuaSystemSet.before(PhysicsSet::SyncBackend).run_if(
                |rapier_config: Res<RapierConfiguration>| rapier_config.physics_pipeline_active,
            ),
        );
        app.add_systems(
            self.schedule,
            (
                update_rigid_body_trackers_system,
                update_proximity_sensors_system,
            )
                .in_set(TnuaPipelineStages::Sensors),
        );
        app.add_systems(
            self.schedule,
            apply_motors_system.in_set(TnuaPipelineStages::Motors),
        );
    }
}

/// `bevy_rapier3d`-specific components required for Tnua to work.
#[derive(Bundle, Default)]
pub struct TnuaRapier3dIOBundle {
    pub velocity: Velocity,
    pub external_force: ExternalForce,
    pub read_mass_properties: ReadMassProperties,
}

/// Add this component to make [`TnuaProximitySensor`] cast a shape instead of a ray.
#[derive(Component)]
pub struct TnuaRapier3dSensorShape(pub Collider);

fn update_rigid_body_trackers_system(
    rapier_config: Res<RapierConfiguration>,
    mut query: Query<(
        &GlobalTransform,
        &Velocity,
        &mut TnuaRigidBodyTracker,
        Option<&TnuaToggle>,
    )>,
) {
    for (transform, velocity, mut tracker, tnua_toggle) in query.iter_mut() {
        match tnua_toggle.copied().unwrap_or_default() {
            TnuaToggle::Disabled => continue,
            TnuaToggle::SenseOnly => {}
            TnuaToggle::Enabled => {}
        }
        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        *tracker = TnuaRigidBodyTracker {
            translation,
            rotation,
            velocity: velocity.linvel,
            angvel: velocity.angvel,
            gravity: rapier_config.gravity,
        };
    }
}

fn get_collider(
    rapier_context: &RapierContext,
    entity: Entity,
) -> Option<&rapier::geometry::Collider> {
    let collider_handle = rapier_context.entity2collider().get(&entity)?;
    rapier_context.colliders.get(*collider_handle)
    //if let Some(owner_collider) = rapier_context.entity2collider().get(&owner_entity).and_then(|handle| rapier_context.colliders.get(*handle)) {
}

#[allow(clippy::type_complexity)]
fn update_proximity_sensors_system(
    rapier_context: Res<RapierContext>,
    mut query: Query<(
        Entity,
        &GlobalTransform,
        &mut TnuaProximitySensor,
        &TnuaRigidBodyTracker,
        Option<&TnuaRapier3dSensorShape>,
        Option<&mut TnuaGhostSensor>,
        Option<&TnuaSubservientSensor>,
        Option<&TnuaToggle>,
    )>,
    ghost_platforms_query: Query<(), With<TnuaGhostPlatform>>,
    other_object_query: Query<(&GlobalTransform, &Velocity)>,
) {
    query.par_iter_mut().for_each(
        |(
            owner_entity,
            transform,
            mut sensor,
            tracker,
            shape,
            mut ghost_sensor,
            subservient,
            tnua_toggle,
        )| {
            match tnua_toggle.copied().unwrap_or_default() {
                TnuaToggle::Disabled => return,
                TnuaToggle::SenseOnly => {}
                TnuaToggle::Enabled => {}
            }
            // cast direction should be the same as gravity direction
            sensor.cast_direction = Dir3::new(tracker.gravity).unwrap_or(Dir3::NEG_Y);

            let cast_origin = transform.transform_point(sensor.cast_origin);
            let cast_direction = sensor.cast_direction;

            struct CastResult {
                entity: Entity,
                proximity: f32,
                intersection_point: Vec3,
                normal: Dir3,
            }

            let owner_entity = if let Some(subservient) = subservient {
                subservient.owner_entity
            } else {
                owner_entity
            };

            let mut query_filter = QueryFilter::new().exclude_rigid_body(owner_entity);
            let owner_solver_groups: InteractionGroups;

            if let Some(owner_collider) = get_collider(&rapier_context, owner_entity) {
                let collision_groups = owner_collider.collision_groups();
                query_filter.groups = Some(CollisionGroups {
                    memberships: Group::from_bits_truncate(collision_groups.memberships.bits()),
                    filters: Group::from_bits_truncate(collision_groups.filter.bits()),
                });
                owner_solver_groups = owner_collider.solver_groups();
            } else {
                owner_solver_groups = InteractionGroups::all();
            }

            let mut already_visited_ghost_entities = HashSet::<Entity>::default();

            let has_ghost_sensor = ghost_sensor.is_some();

            let do_cast = |cast_range_skip: f32,
                           already_visited_ghost_entities: &HashSet<Entity>|
             -> Option<CastResult> {
                let predicate = |other_entity: Entity| {
                    if let Some(other_collider) = get_collider(&rapier_context, other_entity) {
                        if !other_collider.solver_groups().test(owner_solver_groups) {
                            if has_ghost_sensor && ghost_platforms_query.contains(other_entity) {
                                if already_visited_ghost_entities.contains(&other_entity) {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                        }
                        if other_collider.is_sensor() {
                            return false;
                        }
                    }

                    // This fixes https://github.com/idanarye/bevy-tnua/issues/14
                    if let Some(contact) = rapier_context.contact_pair(owner_entity, other_entity) {
                        let same_order = owner_entity == contact.collider1();
                        for manifold in contact.manifolds() {
                            if 0 < manifold.num_points() {
                                let manifold_normal = if same_order {
                                    manifold.local_n2()
                                } else {
                                    manifold.local_n1()
                                };
                                if sensor.intersection_match_prevention_cutoff
                                    < manifold_normal.dot(*cast_direction)
                                {
                                    return false;
                                }
                            }
                        }
                    }
                    true
                };
                let query_filter = query_filter.predicate(&predicate);
                let cast_origin = cast_origin + cast_range_skip * *cast_direction;
                let cast_range = sensor.cast_range - cast_range_skip;
                if let Some(TnuaRapier3dSensorShape(shape)) = shape {
                    let (_, owner_rotation, _) = transform.to_scale_rotation_translation();
                    let owner_rotation = Quat::from_scaled_axis(
                        owner_rotation.to_scaled_axis().dot(*cast_direction) * *cast_direction,
                    );
                    rapier_context
                        .cast_shape(
                            cast_origin,
                            owner_rotation,
                            *cast_direction,
                            shape,
                            ShapeCastOptions {
                                max_time_of_impact: cast_range,
                                target_distance: 0.0,
                                stop_at_penetration: false,
                                compute_impact_geometry_on_penetration: false,
                            },
                            query_filter,
                        )
                        .and_then(|(entity, hit)| {
                            let details = hit.details?;
                            Some(CastResult {
                                entity,
                                proximity: hit.time_of_impact,
                                intersection_point: details.witness1,
                                normal: Dir3::new(details.normal1)
                                    .unwrap_or_else(|_| -cast_direction),
                            })
                        })
                } else {
                    rapier_context
                        .cast_ray_and_get_normal(
                            cast_origin,
                            *cast_direction,
                            cast_range,
                            false,
                            query_filter,
                        )
                        .map(|(entity, hit)| CastResult {
                            entity,
                            proximity: hit.time_of_impact,
                            intersection_point: hit.point,
                            normal: Dir3::new(hit.normal).unwrap_or_else(|_| -cast_direction),
                        })
                }
            };

            let mut cast_range_skip = 0.0;
            if let Some(ghost_sensor) = ghost_sensor.as_mut() {
                ghost_sensor.0.clear();
            }
            sensor.output = 'sensor_output: loop {
                if let Some(CastResult {
                    entity,
                    proximity,
                    intersection_point,
                    normal,
                }) = do_cast(cast_range_skip, &already_visited_ghost_entities)
                {
                    let entity_linvel;
                    let entity_angvel;
                    if let Ok((entity_transform, entity_velocity)) = other_object_query.get(entity)
                    {
                        entity_angvel = entity_velocity.angvel;
                        entity_linvel = entity_velocity.linvel
                            + if 0.0 < entity_angvel.length_squared() {
                                let relative_point =
                                    intersection_point - entity_transform.translation();
                                // NOTE: no need to project relative_point on the rotation plane, it will not
                                // affect the cross product.
                                entity_angvel.cross(relative_point)
                            } else {
                                Vec3::ZERO
                            };
                    } else {
                        entity_angvel = Vec3::ZERO;
                        entity_linvel = Vec3::ZERO;
                    }
                    let sensor_output = TnuaProximitySensorOutput {
                        entity,
                        proximity,
                        normal,
                        entity_linvel,
                        entity_angvel,
                    };
                    if ghost_platforms_query.contains(entity) {
                        cast_range_skip = proximity;
                        already_visited_ghost_entities.insert(entity);
                        if let Some(ghost_sensor) = ghost_sensor.as_mut() {
                            ghost_sensor.0.push(sensor_output);
                        }
                    } else {
                        break 'sensor_output Some(sensor_output);
                    }
                } else {
                    break 'sensor_output None;
                }
            };
        },
    );
}

fn apply_motors_system(
    mut query: Query<(
        &TnuaMotor,
        &mut Velocity,
        &ReadMassProperties,
        &mut ExternalForce,
        Option<&TnuaToggle>,
    )>,
) {
    for (motor, mut velocity, mass_properties, mut external_force, tnua_toggle) in query.iter_mut()
    {
        match tnua_toggle.copied().unwrap_or_default() {
            TnuaToggle::Disabled | TnuaToggle::SenseOnly => {
                *external_force = Default::default();
                return;
            }
            TnuaToggle::Enabled => {}
        }
        if motor.lin.boost.is_finite() {
            velocity.linvel += motor.lin.boost;
        }
        if motor.lin.acceleration.is_finite() {
            external_force.force = motor.lin.acceleration * mass_properties.get().mass;
        }
        if motor.ang.boost.is_finite() {
            velocity.angvel += motor.ang.boost;
        }
        if motor.ang.acceleration.is_finite() {
            external_force.torque =
                motor.ang.acceleration * mass_properties.get().principal_inertia;
        }
    }
}
