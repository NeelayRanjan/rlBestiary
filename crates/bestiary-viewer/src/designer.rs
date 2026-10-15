//! Mutable editor state. Drafts never modify a saved world or its trait cache.
use bestiary_core::genome::{Body, Edge, Genome, JointKind, MuscleGene, MuscleProperties, Route};
use bestiary_sim::{combine::Evaluation, config::Config};
use std::{collections::VecDeque, path::Path};

pub struct Designer {
    pub genome: Genome,
    pub bone: usize,
    pub edge: usize,
    pub muscle: usize,
    pub generation: u64,
    pub hash: [u8; 32],
    pub evaluation: Option<Evaluation>,
    pub pending: bool,
    pub preview: bool,
    pub path: String,
    pub share: String,
    pub status: String,
    pub error: Option<String>,
    pub population: usize,
    pub position: [f32; 2],
}

#[derive(Debug, Default)]
pub struct Validity {
    pub parts: usize,
    pub mass: f32,
    /// Adult basal + sensor + brain + immune cost; excludes measured posture and movement.
    pub basal_watts: f32,
    /// Isometric cost at full activation; excludes mechanical work.
    pub maximum_isometric_watts: f32,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
impl Validity {
    pub fn valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Designer {
    pub fn new(genome: Genome, _config: &Config) -> Self {
        let hash = genome.hash();
        Self {
            genome,
            bone: 0,
            edge: 0,
            muscle: 0,
            generation: 0,
            hash,
            evaluation: None,
            pending: false,
            preview: false,
            path: "exports/designed-creature.genome".into(),
            share: String::new(),
            status: "Edit a preset or the selected creature, then run the combine.".into(),
            error: None,
            population: 8,
            position: [0.0; 2],
        }
    }

    /// Keep invalid values visible and report them rather than silently repairing a draft.
    /// `expand` applies the geometry-aware genesis guarantee whenever the draft is valid.
    pub fn changed(&mut self, _config: &Config) {
        let hash = self.genome.hash();
        if self.hash != hash {
            self.hash = hash;
            self.generation = self.generation.wrapping_add(1);
            self.evaluation = None;
            self.pending = false;
            self.preview = false;
            self.error = None;
            self.status = "Draft changed; combine results must be measured again.".into();
        }
        self.bone = self.bone.min(self.genome.bones.len().saturating_sub(1));
        if let Some(bone) = self.genome.bones.get(self.bone) {
            self.edge = self.edge.min(bone.edges.len().saturating_sub(1));
            self.muscle = self.muscle.min(bone.muscles.len().saturating_sub(1));
        }
    }

    pub fn report(&self, config: &Config) -> Validity {
        let mut report = Validity::default();
        if let Err(error) = config.validate() {
            report.errors.push(error);
            return report;
        }
        if let Err(error) = self.genome.validate(&config.genome) {
            report.errors.push(error);
        }
        for (index, bone) in self.genome.bones.iter().enumerate() {
            if bone
                .size
                .iter()
                .any(|value| !value.is_finite() || *value < config.genome.min_size || *value > 10.0)
            {
                report.errors.push(format!(
                    "Bone {index}: dimensions must be finite and between {} and 10 m.",
                    config.genome.min_size
                ));
            }
            if bone.volume() * bone.density > config.genome.max_mass {
                report.errors.push(format!(
                    "Bone {index}: mass exceeds {} kg.",
                    config.genome.max_mass
                ));
            }
            for (slot, muscle) in bone.muscles.iter().enumerate() {
                if muscle.route.len() > config.genome.max_span {
                    report.errors.push(format!(
                        "Bone {index}, muscle {slot}: span exceeds {} joints.",
                        config.genome.max_span
                    ));
                }
            }
        }
        if !report.valid() {
            return report;
        }
        let body = match self.genome.expand(&config.genome) {
            Ok(body) => body,
            Err(error) => {
                report.errors.push(error);
                return report;
            }
        };
        report.parts = body.parts.len();
        report.mass = body.mass();
        let sensors: usize = self
            .genome
            .bones
            .iter()
            .map(|bone| bone.sensors.len())
            .sum();
        let neurons: usize = self
            .genome
            .bones
            .iter()
            .map(|bone| bone.motor.neurons.len())
            .sum::<usize>()
            + self.genome.ethology.neurons.len();
        report.basal_watts = report.mass * config.world.basal_per_kg
            + sensors as f32 * config.world.sensor_cost
            + neurons as f32 * config.world.brain_cost
            + self.genome.life.immune_investment;
        report.maximum_isometric_watts = body.muscles.iter().map(|muscle| muscle.area).sum::<f32>()
            * config.physics.isometric_cost;
        for (index, part) in body.parts.iter().enumerate() {
            let bone = &self.genome.bones[part.gene];
            for (slot, edge) in bone.edges.iter().enumerate() {
                if part.children[slot].is_some() {
                    continue;
                }
                let mut counts = vec![0; self.genome.bones.len()];
                let mut ancestor = Some(index);
                while let Some(i) = ancestor {
                    counts[body.parts[i].gene] += 1;
                    ancestor = body.parts[i].parent;
                }
                if counts[edge.child] >= self.genome.bones[edge.child].recursion_limit
                    || (edge.terminal_only && counts[part.gene] < bone.recursion_limit)
                {
                    continue;
                }
                report.warnings.push(format!(
                    "Part {index}, joint {slot}: expansion stops at a part/depth/mass/minimum-size bound."
                ));
            }
            for (slot, muscle) in bone.muscles.iter().enumerate() {
                if resolve_route(&body, index, &muscle.route).is_none() {
                    report.warnings.push(format!(
                        "Part {index}, muscle {slot}: endpoint is absent in this bounded instance."
                    ));
                }
            }
        }
        report
    }

    pub fn accept_evaluation(
        &mut self,
        generation: u64,
        result: Result<Evaluation, String>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        match result {
            Ok(evaluation) if evaluation.genome_hash == self.hash => {
                self.pending = false;
                self.status = if evaluation.exploit_suspect {
                    "Combine rejected this body by the free-energy audit.".into()
                } else if evaluation.viability <= 0.0 {
                    "Combine finished; this body fails the standing viability gate.".into()
                } else {
                    "Combine finished; traits and Sprint recording are ready.".into()
                };
                self.error = None;
                self.evaluation = Some(evaluation);
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.pending = false;
                self.error = Some(error);
                self.evaluation = None;
                true
            }
        }
    }

    pub fn preset(&mut self, index: usize, config: &Config) -> Result<(), String> {
        let (name, genome) = bestiary_core::genome::zoo(&config.genome)
            .into_iter()
            .nth(index)
            .ok_or("unknown zoo preset")?;
        self.genome = genome;
        self.bone = self.genome.root;
        self.edge = 0;
        self.muscle = 0;
        self.changed(config);
        self.status = format!("Loaded {name}.");
        Ok(())
    }

    pub fn add_bone(&mut self, parent: usize, config: &Config) -> Result<(), String> {
        self.genome.validate(&config.genome)?;
        if self.genome.bones.len() >= 64 {
            return Err("genome already contains the maximum 64 bone genes".into());
        }
        let mut bone = self
            .genome
            .bones
            .get(parent)
            .ok_or("unknown parent bone")?
            .clone();
        if bone.edges.len() >= 8 {
            return Err("parent already has the maximum eight joints".into());
        }
        bone.edges.clear();
        bone.muscles.clear();
        bone.recursion_limit = 1;
        let child = self.genome.bones.len();
        self.genome.bones.push(bone);
        self.genome.bones[parent].edges.push(new_edge(child));
        self.genome.repair(&config.genome);
        self.bone = child;
        self.edge = 0;
        self.muscle = 0;
        self.changed(config);
        Ok(())
    }

    pub fn add_joint(
        &mut self,
        parent: usize,
        child: usize,
        config: &Config,
    ) -> Result<(), String> {
        self.genome.validate(&config.genome)?;
        if child >= self.genome.bones.len() {
            return Err("unknown child bone".into());
        }
        let bone = self
            .genome
            .bones
            .get_mut(parent)
            .ok_or("unknown parent bone")?;
        if bone.edges.len() >= 8 {
            return Err("parent already has the maximum eight joints".into());
        }
        bone.edges.push(new_edge(child));
        self.bone = parent;
        self.edge = bone.edges.len() - 1;
        self.genome.repair(&config.genome);
        self.changed(config);
        Ok(())
    }

    pub fn remove_bone(&mut self, index: usize, config: &Config) -> Result<(), String> {
        self.genome.validate(&config.genome)?;
        if index == self.genome.root {
            return Err("the root bone cannot be removed".into());
        }
        if index >= self.genome.bones.len() {
            return Err("unknown bone".into());
        }
        // Remove referring joints first, updating edge-slot routes before gene indices move.
        for parent in 0..self.genome.bones.len() {
            let slots: Vec<_> = self.genome.bones[parent]
                .edges
                .iter()
                .enumerate()
                .filter_map(|(slot, edge)| (edge.child == index).then_some(slot))
                .collect();
            for slot in slots.into_iter().rev() {
                remove_edge(&mut self.genome, parent, slot);
            }
        }
        self.genome.bones.remove(index);
        if self.genome.root > index {
            self.genome.root -= 1;
        }
        for bone in &mut self.genome.bones {
            for edge in &mut bone.edges {
                if edge.child > index {
                    edge.child -= 1;
                }
            }
        }
        self.genome.repair(&config.genome);
        self.bone = self.genome.root;
        self.changed(config);
        Ok(())
    }

    pub fn remove_joint(
        &mut self,
        parent: usize,
        slot: usize,
        config: &Config,
    ) -> Result<(), String> {
        self.genome.validate(&config.genome)?;
        if self
            .genome
            .bones
            .get(parent)
            .is_none_or(|bone| slot >= bone.edges.len())
        {
            return Err("unknown joint".into());
        }
        remove_edge(&mut self.genome, parent, slot);
        self.genome.repair(&config.genome);
        self.changed(config);
        Ok(())
    }

    /// Attach between two displayed body instances, including a two-joint sibling span.
    pub fn add_muscle(
        &mut self,
        origin: usize,
        insertion: usize,
        config: &Config,
    ) -> Result<(), String> {
        self.connect_muscle(origin, insertion, [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], config)
    }

    pub fn connect_muscle(
        &mut self,
        origin_part: usize,
        insertion_part: usize,
        origin: [f32; 3],
        insertion: [f32; 3],
        config: &Config,
    ) -> Result<(), String> {
        let body = self.genome.expand(&config.genome)?;
        let route = muscle_route(&body, origin_part, insertion_part, config.genome.max_span)
            .ok_or("muscle endpoints must be distinct reachable parts within the maximum span")?;
        let gene = body.parts[origin_part].gene;
        let bone = &mut self.genome.bones[gene];
        if bone.muscles.len() >= 128 {
            return Err("bone already has the maximum 128 muscle genes".into());
        }
        let muscle = MuscleGene {
            properties: MuscleProperties::default(),
            route,
            origin,
            insertion,
            area: config.physics.fixture_muscle_area,
            output: 0,
        };
        let mut candidate = self.genome.clone();
        candidate.bones[gene].muscles.push(muscle);
        candidate.validate(&config.genome)?;
        self.genome = candidate;
        self.bone = gene;
        self.muscle = self.genome.bones[gene].muscles.len() - 1;
        self.changed(config);
        Ok(())
    }

    pub fn remove_muscle(
        &mut self,
        bone: usize,
        index: usize,
        config: &Config,
    ) -> Result<(), String> {
        let selected = self.genome.bones.get_mut(bone).ok_or("unknown bone")?;
        if index >= selected.muscles.len() {
            return Err("unknown muscle".into());
        }
        selected.muscles.remove(index);
        // Expansion supplies any missing actuated-axis leverage without undoing this deletion.
        self.changed(config);
        Ok(())
    }

    pub fn copy_share(&mut self) {
        self.share = self.genome.to_share();
        self.status = "Share string generated from the current draft.".into();
    }

    pub fn load_share(&mut self, config: &Config) -> Result<(), String> {
        let genome = Genome::from_share(self.share.trim(), &config.genome)?;
        genome.expand(&config.genome)?;
        self.genome = genome;
        self.bone = self.genome.root;
        self.changed(config);
        self.status = "Loaded share string.".into();
        Ok(())
    }

    pub fn save(&mut self, config: &Config) -> Result<(), String> {
        self.genome.expand(&config.genome)?;
        let path = Path::new(&self.path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let bytes = postcard::to_allocvec(&self.genome).map_err(|error| error.to_string())?;
        std::fs::write(path, bytes).map_err(|error| error.to_string())?;
        self.status = format!("Saved {}.", self.path);
        Ok(())
    }

    pub fn load(&mut self, config: &Config) -> Result<(), String> {
        let metadata = std::fs::metadata(&self.path).map_err(|error| error.to_string())?;
        if metadata.len() > 1_000_000 {
            return Err("genome file is too large".into());
        }
        let bytes = std::fs::read(&self.path).map_err(|error| error.to_string())?;
        let (genome, tail): (Genome, &[u8]) =
            postcard::take_from_bytes(&bytes).map_err(|error| error.to_string())?;
        if !tail.is_empty() {
            return Err("trailing genome data".into());
        }
        genome.expand(&config.genome)?;
        self.genome = genome;
        self.bone = self.genome.root;
        self.changed(config);
        self.status = format!("Loaded {}.", self.path);
        Ok(())
    }
}

fn new_edge(child: usize) -> Edge {
    Edge {
        child,
        attachment: [1.0, 0.0, 0.0],
        child_attachment: [-1.0, 0.0, 0.0],
        orientation: [0.0; 3],
        scale: 1.0,
        kind: JointKind::Hinge,
        limits: [[-0.7, 0.7]; 3],
        reflect: false,
        terminal_only: false,
    }
}

fn resolve_route(body: &Body, from: usize, route: &[Route]) -> Option<usize> {
    route.iter().try_fold(from, |part, step| match step {
        Route::Parent => body.parts.get(part)?.parent,
        Route::Child(slot) => body.parts.get(part)?.children.get(*slot).copied().flatten(),
    })
}

pub fn muscle_route(body: &Body, from: usize, to: usize, max_span: usize) -> Option<Vec<Route>> {
    if from >= body.parts.len() || to >= body.parts.len() || from == to {
        return None;
    }
    let mut pending = VecDeque::from([(from, Vec::new())]);
    let mut seen = vec![false; body.parts.len()];
    seen[from] = true;
    while let Some((part, route)) = pending.pop_front() {
        if part == to {
            return Some(route);
        }
        if route.len() >= max_span {
            continue;
        }
        let mut neighbors: Vec<_> = body.parts[part]
            .children
            .iter()
            .enumerate()
            .filter_map(|(slot, child)| child.map(|child| (child, Route::Child(slot))))
            .collect();
        if let Some(parent) = body.parts[part].parent {
            neighbors.push((parent, Route::Parent));
        }
        for (neighbor, step) in neighbors {
            if !seen[neighbor] {
                seen[neighbor] = true;
                let mut next = route.clone();
                next.push(step);
                pending.push_back((neighbor, next));
            }
        }
    }
    None
}

/// Edge slots are route addresses. Resolve routes through all possible gene parents before
/// removing a slot; ambiguous recursive routes are dropped instead of attaching elsewhere.
fn remove_edge(genome: &mut Genome, parent: usize, slot: usize) {
    let original = genome.clone();
    for (gene, bone) in genome.bones.iter_mut().enumerate() {
        bone.muscles.retain_mut(|muscle| {
            let mut locations = vec![gene];
            for step in &mut muscle.route {
                let mut next = Vec::new();
                match step {
                    Route::Child(child_slot) => {
                        let touches = locations.contains(&parent);
                        if touches && (*child_slot == slot || locations.len() > 1) {
                            return false;
                        }
                        for location in &locations {
                            if let Some(edge) = original.bones[*location].edges.get(*child_slot) {
                                next.push(edge.child);
                            }
                        }
                        if touches && *child_slot > slot {
                            *child_slot -= 1;
                        }
                    }
                    Route::Parent => {
                        for (candidate, bone) in original.bones.iter().enumerate() {
                            if bone
                                .edges
                                .iter()
                                .any(|edge| locations.contains(&edge.child))
                            {
                                next.push(candidate);
                            }
                        }
                    }
                }
                next.sort_unstable();
                next.dedup();
                locations = next;
            }
            true
        });
    }
    genome.bones[parent].edges.remove(slot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bestiary_core::genome::Sensor;

    fn fixture() -> (Config, Designer) {
        let config = Config::bundled();
        let editor = Designer::new(Genome::starter(&config.genome), &config);
        (config, editor)
    }

    #[test]
    fn designer_builds_bones_joints_muscles_recursion_reflection_sensors_life_history() {
        let (config, mut editor) = fixture();
        editor.add_bone(1, &config).unwrap();
        let added = editor.bone;
        editor.genome.bones[added].recursion_limit = 2;
        editor.add_joint(added, added, &config).unwrap();
        editor.genome.bones[added].edges[0].reflect = true;
        editor.genome.bones[added].edges[0].kind = JointKind::Ball;
        editor.genome.bones[added]
            .sensors
            .push(Sensor::TerrainForce);
        editor.genome.life.clutch_size = 3;
        editor.changed(&config);
        let body = editor.genome.expand(&config.genome).unwrap();
        assert!(body.actuated_axes_covered());
        assert!(
            body.parts.len()
                > Genome::starter(&config.genome)
                    .expand(&config.genome)
                    .unwrap()
                    .parts
                    .len()
        );
        editor.add_muscle(1, 0, &config).unwrap();
        assert_eq!(
            editor.genome.bones[editor.bone].muscles[editor.muscle].route,
            vec![Route::Parent]
        );
        editor
            .remove_muscle(editor.bone, editor.muscle, &config)
            .unwrap();
        editor.remove_joint(added, 0, &config).unwrap();
        editor.remove_bone(added, &config).unwrap();
        assert!(editor.report(&config).valid());
        assert!(editor
            .genome
            .expand(&config.genome)
            .unwrap()
            .actuated_axes_covered());
    }

    #[test]
    fn designer_live_validity_shows_mass_metabolic_cost_and_bound_violations() {
        let (config, mut editor) = fixture();
        let report = editor.report(&config);
        assert!(report.valid());
        assert_eq!(report.parts, 13);
        assert!(
            report.mass > 0.0 && report.basal_watts > 0.0 && report.maximum_isometric_watts > 0.0
        );
        editor.genome.bones[0].size[0] = 0.0;
        editor.changed(&config);
        let report = editor.report(&config);
        assert!(!report.valid());
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("dimensions")));
        assert_eq!(
            editor.genome.bones[0].size[0], 0.0,
            "invalid draft remains visible"
        );
        let (mut config, editor) = fixture();
        config.genome.max_parts = 2;
        let report = editor.report(&config);
        assert!(report.valid());
        assert_eq!(report.parts, 2);
        assert!(!report.warnings.is_empty());
        config.genome.max_parts = 0;
        assert!(!editor.report(&config).valid());
    }

    #[test]
    fn designer_muscles_connect_reachable_bones_within_span_and_reject_invalid_endpoints() {
        let (config, mut editor) = fixture();
        let body = editor.genome.expand(&config.genome).unwrap();
        let siblings: Vec<_> = body.parts[0].children.iter().flatten().copied().collect();
        assert_eq!(
            muscle_route(&body, siblings[0], siblings[1], 2).unwrap(),
            vec![Route::Parent, Route::Child(1)]
        );
        assert!(muscle_route(&body, siblings[0], siblings[1], 1).is_none());
        assert!(editor.add_muscle(0, 0, &config).is_err());
        assert!(editor.add_muscle(0, usize::MAX, &config).is_err());
        let before = editor.genome.clone();
        assert!(editor
            .connect_muscle(0, 1, [0.0; 3], [1.0, 0.0, 0.0], &config)
            .is_err());
        assert_eq!(editor.genome, before);
        editor
            .add_muscle(siblings[0], siblings[1], &config)
            .unwrap();
        assert!(editor.report(&config).valid());
    }

    #[test]
    fn designer_genesis_guarantee_survives_removal_and_all_joint_types() {
        let (config, mut editor) = fixture();
        for kind in [
            JointKind::Fixed,
            JointKind::Hinge,
            JointKind::Universal,
            JointKind::Ball,
        ] {
            for bone in &mut editor.genome.bones {
                bone.muscles.clear();
                for edge in &mut bone.edges {
                    edge.kind = kind;
                }
            }
            editor.changed(&config);
            assert!(editor
                .genome
                .expand(&config.genome)
                .unwrap()
                .actuated_axes_covered());
        }
        assert!(editor.remove_bone(editor.genome.root, &config).is_err());
        assert!(editor.remove_joint(usize::MAX, 0, &config).is_err());
        assert!(editor.add_joint(0, usize::MAX, &config).is_err());
        assert!(editor.remove_muscle(0, usize::MAX, &config).is_err());
    }

    #[test]
    fn designer_save_load_share_each_of_eight_presets_round_trips_exactly() {
        let (config, mut editor) = fixture();
        let path = std::env::temp_dir().join(format!(
            "bestiary-designer-presets-{}.genome",
            std::process::id()
        ));
        editor.path = path.display().to_string();
        for index in 0..8 {
            editor.preset(index, &config).unwrap();
            let expected = editor.genome.clone();
            editor.save(&config).unwrap();
            editor.copy_share();
            editor.genome.life.clutch_size = 16;
            editor.changed(&config);
            editor.load(&config).unwrap();
            assert_eq!(editor.genome, expected);
            editor.genome.life.clutch_size = 15;
            editor.changed(&config);
            editor.load_share(&config).unwrap();
            assert_eq!(editor.genome, expected);
        }
        assert!(editor.preset(8, &config).is_err());
        editor.share = "invalid share".into();
        let expected = editor.genome.clone();
        assert!(editor.load_share(&config).is_err());
        assert_eq!(editor.genome, expected);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.push(1);
        std::fs::write(&path, bytes).unwrap();
        assert!(editor.load(&config).is_err());
        assert_eq!(editor.genome, expected);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn designer_combine_from_editor_rejects_stale_results_and_invalidate_after_edits() {
        let (config, mut editor) = fixture();
        let evaluation = bestiary_sim::combine::evaluate(&editor.genome, &config).unwrap();
        assert!(editor.accept_evaluation(0, Ok(evaluation.clone())));
        assert!(editor
            .evaluation
            .as_ref()
            .unwrap()
            .trials
            .iter()
            .any(
                |trial| trial.kind == bestiary_sim::combine::TrialKind::Sprint
                    && trial.replay.is_some()
            ));
        editor.genome.life.clutch_size = 3;
        editor.changed(&config);
        assert!(editor.evaluation.is_none());
        assert!(!editor.accept_evaluation(0, Ok(evaluation.clone())));
        assert!(!editor.accept_evaluation(editor.generation, Ok(evaluation)));
        assert!(editor.accept_evaluation(editor.generation, Err("test failure".into())));
        assert_eq!(editor.error.as_deref(), Some("test failure"));
    }

    #[test]
    fn designer_removing_joint_preserves_remaining_muscle_route_targets() {
        let (config, mut editor) = fixture();
        let body = editor.genome.expand(&config.genome).unwrap();
        let second = body.parts[0].children[1].unwrap();
        editor.add_muscle(0, second, &config).unwrap();
        let marker = editor.genome.bones[0].muscles.len() - 1;
        editor.genome.bones[0].muscles[marker].area = 0.12345;
        editor.remove_joint(0, 0, &config).unwrap();
        let muscle = editor.genome.bones[0]
            .muscles
            .iter()
            .find(|muscle| muscle.area == 0.12345)
            .unwrap();
        assert_eq!(muscle.route, vec![Route::Child(0)]);
        assert!(editor
            .genome
            .expand(&config.genome)
            .unwrap()
            .actuated_axes_covered());
    }
}
