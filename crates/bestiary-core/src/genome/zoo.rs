use super::*;
/// Hand-authored genomes; niches are labels, not scripted behavior or fitness bonuses.
pub fn zoo(limits: &Limits) -> Vec<(&'static str, Genome)> {
    let grazer = Genome::starter(limits);
    let mut sprinter = grazer.clone();
    for b in sprinter.bones.iter_mut().skip(1) {
        // Retain the inherited hip/knee timing with a modest reach change;
        // larger rescaling can destabilize contact in the slope trial.
        b.size[1] *= 1.1;
    }
    let mut predator = grazer.clone();
    predator.life.plant_efficiency = 0.1;
    for b in &mut predator.bones {
        for m in &mut b.muscles {
            // Increase contractile area without changing joint geometry or
            // exceeding the inherited gait's tested contact behavior.
            m.area *= 1.2;
        }
    }
    let mut scavenger = grazer.clone();
    scavenger.life.plant_efficiency = 0.35;
    scavenger.life.immune_investment = 0.5;
    let mut burrower = grazer.clone();
    burrower.bones[0].size[1] = 0.18;
    for b in burrower.bones.iter_mut().skip(1) {
        b.size[1] *= 0.65;
    }
    let mut swimmer = Genome::rigid_limb_fixture(limits);
    swimmer.bones[0].shape = Shape::Capsule;
    swimmer.bones[0].size = [0.3, 0.9, 0.3];
    swimmer.bones[0].density = 2.2;
    for b in swimmer.bones.iter_mut().skip(1) {
        b.size = [0.06, 0.4, 0.18];
    }
    let mut serpent = grazer.clone();
    serpent.bones.truncate(1);
    serpent.bones[0].size = [0.25, 0.15, 0.15];
    serpent.bones[0].muscles.clear();
    serpent.bones[0].edges = vec![Edge {
        child: 1,
        attachment: [1.0, 0.0, 0.0],
        child_attachment: [-1.0, 0.0, 0.0],
        orientation: [0.0, 0.0, -0.8],
        scale: 1.0,
        kind: JointKind::Fixed,
        limits: [[0.0; 2]; 3],
        reflect: false,
        terminal_only: false,
    }];
    // A raised head and neck lead into one recursively repeated, unbranched
    // trunk. The trunk lies on the floor while the root remains above it.
    let mut neck = serpent.bones[0].clone();
    neck.size = [0.45, 0.12, 0.12];
    neck.edges[0].child = 2;
    neck.edges[0].orientation = [std::f32::consts::FRAC_PI_2, 0.0, 0.8];
    let mut trunk = serpent.bones[0].clone();
    trunk.size = [0.3, 0.16, 0.2];
    trunk.recursion_limit = 7;
    trunk.edges = vec![Edge {
        child: 2,
        attachment: [1.0, 0.0, 0.0],
        child_attachment: [-1.0, 0.0, 0.0],
        orientation: [0.0; 3],
        scale: 1.0,
        kind: JointKind::Hinge,
        limits: [[-0.3, 0.3]; 3],
        reflect: false,
        terminal_only: false,
    }];
    for (output, sign) in [-1.0, 1.0].into_iter().enumerate() {
        trunk.muscles.push(MuscleGene {
            properties: MuscleProperties::default(),
            route: vec![Route::Child(0)],
            origin: [0.8, sign, 0.0],
            insertion: [-0.8, sign, 0.0],
            area: 0.015,
            output,
        });
    }
    serpent.bones.extend([neck, trunk]);
    let mut drifter = Genome::rigid_limb_fixture(limits);
    drifter.bones[0].shape = Shape::Capsule;
    drifter.bones[0].size = [0.4; 3];
    for (i, e) in drifter.bones[0].edges.iter_mut().enumerate() {
        let angle = std::f32::consts::TAU * i as f32 / 6.0;
        let x = libm::cosf(angle);
        let z = libm::sinf(angle);
        let m = x.abs().max(z.abs());
        e.attachment = [x / m, 0.0, z / m];
        // Genome Euler angles compose Z * Y * X. Decompose the intended
        // radial Y rotation followed by the same outward lean on every limb.
        let lean = 0.35;
        e.orientation = [
            libm::atan2f(-z * libm::sinf(lean), x),
            libm::asinf(-z * libm::cosf(lean)),
            libm::atan2f(libm::sinf(lean), x * libm::cosf(lean)),
        ];
        e.limits = [[-0.3, 0.3]; 3];
    }
    for (i, bone) in drifter.bones.iter_mut().skip(1).enumerate() {
        let angle = std::f32::consts::TAU * i as f32 / 6.0;
        bone.shape = Shape::Capsule;
        bone.size = [0.08, 0.55, 0.08];
        bone.motor = Brain::oscillator(0.0);
        bone.motor.frequency = 0.5;
        bone.muscles.clear();
        for (output, sign) in [-1.0, 1.0].into_iter().enumerate() {
            bone.muscles.push(MuscleGene {
                properties: MuscleProperties::default(),
                route: vec![Route::Parent],
                origin: [sign, 0.0, 0.0],
                insertion: [0.4 * libm::cosf(angle), sign, 0.4 * libm::sinf(angle)],
                area: 0.02,
                output,
            });
        }
    }
    let mut result = vec![
        ("grazer", grazer),
        ("sprinter", sprinter),
        ("ambush predator", predator),
        ("scavenger", scavenger),
        ("burrower", burrower),
        ("swimmer", swimmer),
        ("serpent", serpent),
        ("radial drifter", drifter),
    ];
    for (_, g) in &mut result {
        g.repair(limits);
    }
    result
}
/// A twelve-part genome for the mandatory combine benchmark.
pub fn benchmark_genome(limits: &Limits) -> Genome {
    let mut g = Genome::rigid_limb_fixture(limits);
    let mut parent = 0;
    for _ in 0..5 {
        let child = g.bones.len();
        let mut bone = g.bones[1].clone();
        bone.size = [0.08; 3];
        bone.muscles.clear();
        bone.edges.clear();
        g.bones.push(bone);
        g.bones[parent].edges.push(Edge {
            child,
            attachment: [1.0, 0.0, 0.0],
            child_attachment: [-1.0, 0.0, 0.0],
            orientation: [0.0; 3],
            scale: 1.0,
            kind: JointKind::Fixed,
            limits: [[0.0, 0.0]; 3],
            reflect: false,
            terminal_only: false,
        });
        parent = child;
    }
    g.repair(limits);
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> Limits {
        #[derive(Deserialize)]
        struct Config {
            genome: Limits,
        }
        ron::from_str::<Config>(include_str!("../../../../config/world.ron"))
            .unwrap()
            .genome
    }

    #[test]
    fn authored_serpent_has_deep_recursion_and_no_limb_branches() {
        let limits = limits();
        let (_, serpent) = zoo(&limits)
            .into_iter()
            .find(|(name, _)| *name == "serpent")
            .unwrap();
        let body = serpent.expand(&limits).unwrap();
        assert_eq!(body.parts.len(), 9);
        assert!(body
            .parts
            .iter()
            .all(|p| p.children.iter().flatten().count() <= 1));
        assert_eq!(body.parts.iter().map(|p| p.depth).max(), Some(8));
        assert!(serpent
            .bones
            .iter()
            .enumerate()
            .any(|(i, b)| b.recursion_limit >= 7 && b.edges.iter().any(|e| e.child == i)));
        assert!(body.actuated_axes_covered());
    }

    #[test]
    fn radial_drifter_has_sixfold_support_geometry() {
        let limits = limits();
        let (_, drifter) = zoo(&limits)
            .into_iter()
            .find(|(name, _)| *name == "radial drifter")
            .unwrap();
        let body = drifter.expand(&limits).unwrap();
        assert_eq!(body.parts.len(), 7);
        assert_eq!(body.parts[0].shape, Shape::Capsule);
        assert_eq!(body.parts[0].size, [0.4; 3]);
        let reference = body.parts[1].pose.translation.vector;
        let axis = body.parts[1].pose.rotation * nalgebra::Vector3::y();
        for (i, part) in body.parts.iter().skip(1).enumerate() {
            let angle = std::f32::consts::TAU * i as f32 / 6.0;
            let (s, c) = (libm::sinf(angle), libm::cosf(angle));
            let rotate = |v: nalgebra::Vector3<f32>| {
                nalgebra::Vector3::new(c * v.x - s * v.z, v.y, s * v.x + c * v.z)
            };
            assert!((part.pose.translation.vector - rotate(reference)).norm() < 1e-5);
            assert!((part.pose.rotation * nalgebra::Vector3::y() - rotate(axis)).norm() < 1e-5);
            assert_eq!(part.size, body.parts[1].size);
            assert_eq!(part.shape, body.parts[1].shape);
        }
        assert!(body.actuated_axes_covered());
    }
}
