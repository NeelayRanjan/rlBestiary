use super::*;
use proptest::prelude::*;
fn limits() -> Limits {
    #[derive(Deserialize)]
    struct Config {
        genome: Limits,
    }
    ron::from_str::<Config>(include_str!("../../../../config/world.ron"))
        .unwrap()
        .genome
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn mutation_sequences_are_valid_bounded_and_axes_spanned(seed in any::<u64>(),ops in prop::collection::vec(0usize..23,1..40)) {
        let limits=limits();let mut g=Genome::starter(&limits);
        for (tick,op) in ops.into_iter().enumerate() {
            let (next,record)=g.mutate(Operator::ALL[op],seed,tick as u64,&limits);
            prop_assert_eq!(record.parent,g.hash());prop_assert_eq!(record.child,next.hash());
            let b=next.expand(&limits).unwrap();
            prop_assert!(b.parts.len()<=32 && b.parts.iter().all(|p|p.depth<=12));
            prop_assert!(b.mass()<=limits.max_mass+0.001 && b.actuated_axes_covered());
            prop_assert_eq!(Genome::from_share(&next.to_share(),&limits).unwrap(),next.clone());
            g=next;
        }
    }
    #[test]
    fn crossover_always_valid_and_distance_symmetric(seed in any::<u64>()) {
        let limits=limits();let a=Genome::starter(&limits);
        let (b,_)=a.mutate(Operator::LifeHistory,seed,0,&limits);
        let c=a.crossover(&b,seed,&limits);let body=c.expand(&limits).unwrap();
        prop_assert!(body.actuated_axes_covered());prop_assert!(body.parts.len()<=32);
        prop_assert_eq!(a.compatibility(&b,&[],&[]),b.compatibility(&a,&[],&[]));
        prop_assert_eq!(a.compatibility(&a,&[],&[]),0.0);
        if a!=b {prop_assert!(a.compatibility(&b,&[],&[])>0.0);}
    }
}
#[test]
fn recursive_cycles_stop_cleanly_at_bounds_and_terminal_edges_attach() {
    let limits = limits();
    let mut g = Genome::starter(&limits);
    g.bones[0].recursion_limit = 12;
    let mut self_edge = g.bones[0].edges[0].clone();
    self_edge.child = 0;
    self_edge.scale = 0.8;
    g.bones[0].edges = vec![self_edge];
    g.repair(&limits);
    let b = g.expand(&limits).unwrap();
    assert!(b.parts.len() > 1 && b.parts.len() <= 32);
    assert!(b.parts.iter().all(|p| p.depth <= 12));
    assert!(b.actuated_axes_covered());
    let mut small = limits.clone();
    small.max_parts = 1;
    assert_eq!(g.expand(&small).unwrap().parts.len(), 1);
    let mut terminal = g.bones[0].edges[0].clone();
    let mut terminal_bone = g.bones[0].clone();
    terminal_bone.edges.clear();
    terminal_bone.muscles.clear();
    g.bones.push(terminal_bone);
    terminal.child = 1;
    terminal.terminal_only = true;
    g.bones[0].edges.push(terminal);
    g.bones[0].recursion_limit = 3;
    g.repair(&limits);
    let b = g.expand(&limits).unwrap();
    assert_eq!(b.parts.iter().filter(|p| p.gene == 1).count(), 1);
}
#[test]
fn reflection_carries_neurons_and_spanning_muscles() {
    let limits = limits();
    let mut g = Genome::starter(&limits);
    g.bones[0].edges.truncate(1);
    let mut edge = g.bones[0].edges[0].clone();
    edge.attachment[2] = -edge.attachment[2];
    edge.reflect = true;
    g.bones[0].edges.push(edge);
    g.repair(&limits);
    let body = g.expand(&limits).unwrap();
    assert_eq!(body.parts.len(), 5);
    assert_eq!(body.parts[1].gene, body.parts[3].gene);
    assert_eq!(
        body.parts[1].pose.translation.z,
        -body.parts[3].pose.translation.z
    );
    assert!(body.actuated_axes_covered());
    let brain = &g.bones[body.parts[3].gene].motor;
    let mut state = brain.state();
    brain.step(&mut state, &[], &[], &[], 0.1);
    assert!(state.values.iter().any(|x| *x != 0.0));
}
#[test]
fn share_and_configuration_negative_cases() {
    let mut l = limits();
    assert!(Genome::from_share("not valid!", &l).is_err());
    let mut g = Genome::starter(&l);
    g.root = 100;
    assert!(g.validate(&l).is_err());
    l.max_parts = 33;
    assert!(l.validate().is_err());
    l = limits();
    l.min_size = f32::NAN;
    assert!(l.validate().is_err());
}
