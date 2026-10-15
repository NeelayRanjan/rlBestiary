use super::*;
/// Hand-authored genomes; niches are labels, not scripted behavior or fitness bonuses.
pub fn zoo(limits: &Limits) -> Vec<(&'static str, Genome)> {
    let grazer = Genome::starter(limits);
    let mut sprinter = grazer.clone();
    for b in sprinter.bones.iter_mut().skip(1) {
        b.size[1] *= 1.2;
        b.motor.frequency = 2.0;
    }
    let mut predator = grazer.clone();
    predator.life.plant_efficiency = 0.1;
    for b in &mut predator.bones {
        b.size = b.size.map(|x| x * 1.2);
        for m in &mut b.muscles {
            m.area *= 1.5;
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
    let mut swimmer = grazer.clone();
    swimmer.bones[0].shape = Shape::Capsule;
    swimmer.bones[0].size = [0.3, 0.9, 0.3];
    swimmer.bones[0].density = 2.2;
    for b in swimmer.bones.iter_mut().skip(1) {
        b.size = [0.06, 0.4, 0.18];
    }
    let mut serpent = grazer.clone();
    serpent.bones.truncate(1);
    serpent.bones[0].size = [0.3, 0.16, 0.16];
    serpent.bones[0].recursion_limit = 8;
    serpent.bones[0].muscles.clear();
    serpent.bones[0].edges = vec![Edge {
        child: 0,
        attachment: [1.0, 0.0, 0.0],
        child_attachment: [-1.0, 0.0, 0.0],
        orientation: [0.0; 3],
        scale: 0.95,
        kind: JointKind::Hinge,
        limits: [[-0.4, 0.4]; 3],
        reflect: false,
        terminal_only: false,
    }];
    let mut drifter = grazer.clone();
    drifter.bones[0].size = [0.4, 0.2, 0.4];
    for (i, e) in drifter.bones[0].edges.iter_mut().enumerate() {
        let angle = std::f32::consts::TAU * i as f32 / 6.0;
        let x = libm::cosf(angle);
        let z = libm::sinf(angle);
        let m = x.abs().max(z.abs());
        e.attachment = [x / m, 0.0, z / m];
        e.orientation = [0.0, angle, std::f32::consts::FRAC_PI_2];
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
    let mut g = Genome::starter(limits);
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
