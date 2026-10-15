//! cargo run --release -p bestiary-sim --example gait_probe -- [seconds] [output-prefix]
use bestiary_core::genome::Genome;
use bestiary_sim::{config::Config, locomotion::measure};
fn main() {
    let c = Config::bundled();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let seconds = args
        .first()
        .map(|s| s.parse().expect("measurement seconds"))
        .unwrap_or(8.0);
    let g = Genome::starter(&c.genome);
    let (report, replay) = measure(&g, &c, 2.0, seconds, true).expect("active gait");
    println!("coordinated founder: {report:#?}");
    println!(
        "passive ablation: {:#?}",
        measure(&g, &c, 2.0, seconds, false)
            .expect("passive gait")
            .0
    );
    println!(
        "rigid-limb baseline: {:#?}",
        measure(
            &Genome::rigid_limb_fixture(&c.genome),
            &c,
            2.0,
            seconds,
            true
        )
        .expect("baseline gait")
        .0
    );
    if let Some(prefix) = args.get(1) {
        std::fs::write(
            format!("{prefix}.genome"),
            postcard::to_allocvec(&g).unwrap(),
        )
        .unwrap();
        std::fs::write(format!("{prefix}.rpl"), replay.encode().unwrap()).unwrap();
        std::fs::write(
            format!("{prefix}.ron"),
            ron::ser::to_string_pretty(&report, ron::ser::PrettyConfig::default()).unwrap(),
        )
        .unwrap();
    }
}
