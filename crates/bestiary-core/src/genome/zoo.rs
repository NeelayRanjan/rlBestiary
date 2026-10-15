use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ZooBehavior {
    maturity: u64,
    senescence: u64,
    clutch_size: u8,
    offspring_fraction: f32,
    burrower_clutch_size: u8,
    burrower_offspring_fraction: f32,
    predator_offspring_fraction: f32,
    forager: Brain,
    predator: Brain,
    scavenger: Brain,
}
impl ZooBehavior {
    fn parse(source: &str) -> Result<Self, String> {
        let genes: Self = ron::from_str(source).map_err(|error| error.to_string())?;
        if genes.maturity == 0
            || genes.senescence <= genes.maturity
            || !(1..=16).contains(&genes.clutch_size)
            || !(0.01..=0.5).contains(&genes.offspring_fraction)
            || !(1..=16).contains(&genes.burrower_clutch_size)
            || !(0.01..=0.5).contains(&genes.burrower_offspring_fraction)
            || !(0.01..=0.5).contains(&genes.predator_offspring_fraction)
            || [&genes.forager, &genes.predator, &genes.scavenger]
                .into_iter()
                .any(|brain| !brain.valid() || brain.neurons.len() != 13)
        {
            return Err("invalid zoo lifecycle or world behavior genes".into());
        }
        Ok(genes)
    }
}

/// Hand-authored genomes; niches are labels, not scripted behavior or fitness bonuses.
pub fn zoo(limits: &Limits) -> Vec<(&'static str, Genome)> {
    let behavior = ZooBehavior::parse(include_str!("../../../../config/zoo.ron"))
        .expect("valid embedded zoo behavior");
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
    for (name, g) in &mut result {
        g.ethology = match *name {
            "ambush predator" => behavior.predator.clone(),
            "scavenger" => behavior.scavenger.clone(),
            _ => behavior.forager.clone(),
        };
        g.life.maturity = behavior.maturity;
        g.life.senescence = behavior.senescence;
        g.life.clutch_size = behavior.clutch_size;
        g.life.offspring_fraction = behavior.offspring_fraction;
        if *name == "ambush predator" {
            g.life.offspring_fraction = behavior.predator_offspring_fraction;
        }
        if *name == "burrower" {
            g.life.clutch_size = behavior.burrower_clutch_size;
            g.life.offspring_fraction = behavior.burrower_offspring_fraction;
        }
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
    fn zoo_behavior_schema_rejects_unknown_invalid_and_missing_genes() {
        let source = include_str!("../../../../config/zoo.ron");
        assert!(ZooBehavior::parse(source).is_ok());
        for changed in [
            source.replace("maturity: 2000", "maturity: 0"),
            source.replace("senescence: 20000", "senescence: 2000"),
            source.replace("clutch_size: 2", "clutch_size: 0"),
            source.replace("clutch_size: 2", "clutch_size: 17"),
            source.replace("offspring_fraction: 0.15", "offspring_fraction: 0.0"),
            source.replace("offspring_fraction: 0.15", "offspring_fraction: 0.6"),
            source.replace("burrower_clutch_size: 1", "burrower_clutch_size: 0"),
            source.replace(
                "burrower_offspring_fraction: 0.5",
                "burrower_offspring_fraction: 1.0",
            ),
            source.replace(
                "predator_offspring_fraction: 0.3",
                "predator_offspring_fraction: -0.1",
            ),
            source.replace("frequency: 0.001", "frequency: -1.0"),
            source.replace("weight: 1.0", "weight: 17.0"),
            source.replace("maturity: 2000", "unknown: 2000"),
        ] {
            assert!(ZooBehavior::parse(&changed).is_err());
        }
    }

    #[test]
    fn authored_zoo_has_inheritable_feeding_and_mating_preferences() {
        for (name, genome) in zoo(&limits()) {
            assert_eq!(genome.ethology.neurons.len(), 13, "{name}");
            let mut sensors = vec![0.0; 20];
            sensors[0] = 0.9;
            sensors[2] = 1.0;
            sensors[3] = 1.0;
            sensors[4] = 1.0;
            sensors[11] = 1.0;
            let mut brain = genome.ethology.state();
            genome.ethology.step(&mut brain, &sensors, &[], &[], 0.0);
            assert!(brain.values[3] > brain.values[8], "{name}: feeding");
            assert!(brain.values[7] > brain.values[8], "{name}: mating");
            assert!(
                brain.values[4] > brain.values[3],
                "{name}: approach a compatible mate while fed"
            );
            assert!(
                brain.values[6] < brain.values[3],
                "{name}: avoid kin attack"
            );
            sensors[3] = 0.15;
            genome.ethology.step(&mut brain, &sensors, &[], &[], 10.0);
            assert!(
                brain.values[4] < brain.values[3],
                "{name}: juveniles seek food instead of mates"
            );
            sensors[3] = 1.0;
            sensors[0] = 0.1;
            sensors[11] = 0.0;
            genome.ethology.step(&mut brain, &sensors, &[], &[], 10.0);
            assert!(
                brain.values[4] < brain.values[3],
                "{name}: no pursuit of unlike neighbors"
            );
            if name == "ambush predator" {
                assert!(
                    brain.values[6] > brain.values[3],
                    "hungry predator attacks unlike neighbors"
                );
            }
            genome.validate(&limits()).unwrap();
        }
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
