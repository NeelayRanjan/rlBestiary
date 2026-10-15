//! Compile an expanded genome into Rapier bodies and execute distributed brains.
use crate::{config::Config, physics::Arena};
use bestiary_core::{
    brain::BrainState,
    genome::{local_point, Body, Genome, Sensor, Shape},
};
use rapier3d::prelude::*;

pub struct Creature {
    pub genome: Genome,
    pub body: Body,
    pub brains: Vec<BrainState>,
    pub arena: Arena,
}
impl Creature {
    pub fn new(genome: Genome, config: &Config) -> Result<Self, String> {
        let body = genome.expand(&config.genome)?;
        let mut arena = Arena::new(config.physics.clone())?;
        let mut lowest = 0.0f32;
        for p in &body.parts {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        lowest = lowest.min((p.pose * local_point(p.size, [x, y, z], false)).y);
                    }
                }
            }
        }
        for p in &body.parts {
            let bone = &genome.bones[p.gene];
            let mut pose = p.pose;
            pose.translation.y += -lowest + 0.01;
            let h = arena.bodies.insert(
                RigidBodyBuilder::dynamic()
                    .position(pose)
                    .can_sleep(false)
                    .ccd_enabled(true),
            );
            let collider = match bone.shape {
                Shape::Box => {
                    ColliderBuilder::cuboid(p.size[0] * 0.5, p.size[1] * 0.5, p.size[2] * 0.5)
                }
                Shape::Capsule => {
                    let radius = p.size[0].min(p.size[2]) * 0.5;
                    ColliderBuilder::capsule_y((p.size[1] * 0.5 - radius).max(0.0), radius)
                }
            }
            .density(bone.density)
            .friction(config.physics.friction)
            .restitution(0.0);
            arena
                .colliders
                .insert_with_parent(collider, h, &mut arena.bodies);
            arena.parts.push(h);
        }
        for (i, p) in body.parts.iter().enumerate().skip(1) {
            let parent = p.parent.expect("expanded child");
            let pp = &body.parts[parent];
            let e = p.incoming.as_ref().expect("incoming");
            let mut locked = JointAxesMask::LOCKED_FIXED_AXES;
            for axis in e.kind.axes() {
                locked.remove(
                    [
                        JointAxesMask::ANG_X,
                        JointAxesMask::ANG_Y,
                        JointAxesMask::ANG_Z,
                    ][*axis],
                );
            }
            let frame1 = Isometry::from_parts(
                Translation::from(local_point(pp.size, e.attachment, pp.reflected).coords),
                pp.pose.rotation.inverse() * p.pose.rotation,
            );
            let frame2 = Isometry::translation(
                local_point(p.size, e.child_attachment, p.reflected).x,
                local_point(p.size, e.child_attachment, p.reflected).y,
                local_point(p.size, e.child_attachment, p.reflected).z,
            );
            let mut joint = GenericJointBuilder::new(locked)
                .local_frame1(frame1)
                .local_frame2(frame2)
                .contacts_enabled(false);
            for axis in e.kind.axes() {
                joint = joint.limits(
                    [JointAxis::AngX, JointAxis::AngY, JointAxis::AngZ][*axis],
                    e.limits[*axis],
                );
            }
            arena
                .joints
                .insert(arena.parts[parent], arena.parts[i], joint, true);
        }
        for m in &body.muscles {
            arena.add_muscle(
                arena.parts[m.origin_part],
                arena.parts[m.insertion_part],
                m.origin.into(),
                m.insertion.into(),
                m.area,
                0.0,
            );
            arena.muscles.last_mut().expect("just inserted").properties = m.properties.clone();
        }
        let brains = body
            .parts
            .iter()
            .map(|p| genome.bones[p.gene].motor.state())
            .collect();
        Ok(Self {
            genome,
            body,
            brains,
            arena,
        })
    }
    pub fn step(&mut self) {
        let previous: Vec<_> = self.brains.iter().map(|b| b.values.clone()).collect();
        for (i, p) in self.body.parts.iter().enumerate() {
            let bone = &self.genome.bones[p.gene];
            let rigid = &self.arena.bodies[self.arena.parts[i]];
            let parent = p.parent.map(|j| &self.arena.bodies[self.arena.parts[j]]);
            let relative = parent.map_or(*rigid.rotation(), |b| {
                b.rotation().inverse() * rigid.rotation()
            });
            let q = relative.quaternion();
            let norm = libm::sqrtf(q.i * q.i + q.j * q.j + q.k * q.k);
            let angle = if norm > 1e-6 {
                2.0 * libm::atan2f(norm, q.w) / norm
            } else {
                0.0
            };
            let angular = [q.i * angle, q.j * angle, q.k * angle];
            let velocity = rigid.rotation().inverse()
                * (rigid.angvel() - parent.map_or(Vector::zeros(), |b| *b.angvel()));
            let gravity = rigid.rotation().inverse() * Vector::y();
            let contact = self.arena.contact_force(self.arena.parts[i]);
            let owned: Vec<_> = self
                .arena
                .muscles
                .iter()
                .filter(|m| m.origin_body == self.arena.parts[i])
                .collect();
            let mut sensors: Vec<f32> = bone
                .sensors
                .iter()
                .map(|sensor| match *sensor {
                    Sensor::JointAngle(axis) => angular.get(axis).copied().unwrap_or(0.0),
                    Sensor::JointVelocity(axis) => velocity.get(axis).copied().unwrap_or(0.0),
                    Sensor::Gravity(axis) => gravity.get(axis).copied().unwrap_or(0.0),
                    Sensor::Contact => {
                        if contact > 0.0 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    Sensor::ContactForce => contact,
                    Sensor::MuscleLength(mi) => owned.get(mi).map_or(0.0, |m| {
                        let (a, b) = self.arena.endpoints(m);
                        (b - a).norm() / m.rest
                    }),
                    Sensor::MuscleVelocity(mi) => owned.get(mi).map_or(0.0, |m| {
                        let (a, b) = self.arena.endpoints(m);
                        let direction = (b - a).try_normalize(1e-6).unwrap_or_default();
                        (self.arena.bodies[m.insertion_body].velocity_at_point(&b)
                            - rigid.velocity_at_point(&a))
                        .dot(&direction)
                    }),
                })
                .collect();
            // Body orientation and body velocity are the final six global sensor values.
            sensors.extend(gravity.iter());
            sensors.extend(self.arena.bodies[self.arena.parts[0]].linvel().iter());
            let mut children = vec![0.0; 32];
            let mut n = 0.0;
            for child in p.children.iter().flatten() {
                for (j, v) in previous[*child].iter().enumerate() {
                    children[j] += v;
                }
                n += 1.0;
            }
            if n > 0.0 {
                children.iter_mut().for_each(|x| *x /= n);
            }
            let parent_values = p.parent.map_or(&[][..], |p| previous[p].as_slice());
            bone.motor.step(
                &mut self.brains[i],
                &sensors,
                parent_values,
                &children,
                self.arena.tick as f32 * crate::PHYSICS_DT,
            );
        }
        let activations: Vec<_> = self
            .body
            .muscles
            .iter()
            .map(|m| self.brains[m.origin_part].values[m.output].clamp(0.0, 1.0))
            .collect();
        self.arena.step(&activations);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expanded_body_instantiates_with_no_joint_motors_and_repeats() {
        let config = Config::bundled();
        let g = Genome::starter(&config.genome);
        let mut a = Creature::new(g.clone(), &config).unwrap();
        let mut b = Creature::new(g, &config).unwrap();
        assert_eq!(a.arena.parts.len(), 7);
        for (_, joint) in a.arena.joints.iter() {
            assert!(joint.data.motor_axes.is_empty());
        }
        for _ in 0..120 {
            a.step();
            b.step();
        }
        assert_eq!(a.arena.poses(), b.arena.poses());
        assert_eq!(a.brains, b.brains);
        assert!(a
            .arena
            .poses()
            .iter()
            .all(|p| p.translation.iter().all(|v| v.is_finite())));
    }
}
