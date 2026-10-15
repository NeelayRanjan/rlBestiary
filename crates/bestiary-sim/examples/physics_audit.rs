use bestiary_core::genome::{zoo, Genome};
use bestiary_sim::{combine::evaluate, config::Config, genome::Creature, PHYSICS_DT};

fn inspect(name: &str, g: &Genome, c: &Config) {
    let result = evaluate(g, c).unwrap();
    println!(
        "{name}: parts={} viability={:.6} speed={:.6} swim={:.6} exploit={}",
        result.traits.parts,
        result.viability,
        result.traits.speed,
        result.traits.swim_speed,
        result.exploit_suspect
    );
    for driven in [true, false] {
        let mut creature = Creature::new(g.clone(), c).unwrap();
        let root = creature.arena.parts[0];
        let mut last = *creature.arena.bodies[root].translation();
        let start = last;
        let mut min_center = last.y;
        let mut min_bottom = f32::INFINITY;
        let mut paths = [0.0f32; 2];
        let mut peaks = [0.0f32; 2];
        let mut contacts = 0;
        for tick in 0..480 {
            if driven {
                creature.step();
            } else {
                creature
                    .arena
                    .step(&vec![0.0; creature.arena.muscles.len()]);
            }
            let body = &creature.arena.bodies[root];
            let p = *body.translation();
            min_center = min_center.min(p.y);
            for collider in body.colliders() {
                min_bottom =
                    min_bottom.min(creature.arena.colliders[*collider].compute_aabb().mins.y);
            }
            contacts += usize::from(creature.arena.contact_force(root) > 0.0);
            let delta = libm::sqrtf((p.x - last.x).powi(2) + (p.z - last.z).powi(2));
            paths[tick / 240] += delta;
            peaks[tick / 240] = peaks[tick / 240].max(delta / PHYSICS_DT);
            last = p;
        }
        println!("  active={driven} min_center={min_center:.6} min_bottom={min_bottom:.6} root_contact_steps={contacts} path_first_second={:.6} path_second_second={:.6} peak_first={:.6} peak_second={:.6} net_x={:.6} net_z={:.6}", paths[0], paths[1], peaks[0], peaks[1], last.x-start.x, last.z-start.z);
    }
}

fn main() {
    let c = Config::bundled();
    inspect("starter", &Genome::starter(&c.genome), &c);
    for (name, g) in zoo(&c.genome) {
        inspect(name, &g, &c);
    }
    if let Some(path) = std::env::args().nth(1) {
        let bytes = std::fs::read(&path).unwrap();
        let g: Genome = postcard::from_bytes(&bytes).unwrap();
        inspect(&path, &g, &c);
    }
    let mut inert = Genome::starter(&c.genome);
    inert.bones[0].edges.clear();
    inert.bones[0].muscles.clear();
    inert.bones.truncate(1);
    inert.repair(&c.genome);
    inspect("inert-root-box", &inert, &c);
}
