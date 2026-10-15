use super::*;
use nalgebra::{Isometry3, Point3, Quaternion, Translation3, UnitQuaternion};
use rand::Rng;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub gene: usize,
    pub parent: Option<usize>,
    pub incoming: Option<Edge>,
    pub children: Vec<Option<usize>>,
    pub pose: Isometry3<f32>,
    pub size: [f32; 3],
    pub scale: f32,
    pub reflected: bool,
    pub mass: f32,
    pub depth: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExpandedMuscle {
    pub properties: MuscleProperties,
    pub origin_part: usize,
    pub insertion_part: usize,
    pub origin: [f32; 3],
    pub insertion: [f32; 3],
    pub area: f32,
    pub output: usize,
    /// (child part, actuated axis), measured from nonzero moment arm.
    pub covers: Vec<(usize, usize)>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Body {
    pub parts: Vec<Part>,
    pub muscles: Vec<ExpandedMuscle>,
}
fn rotation(euler: [f32; 3], reflected: bool) -> UnitQuaternion<f32> {
    let [x, y, z] = euler.map(|v| v * 0.5);
    let (sx, cx) = (libm::sinf(x), libm::cosf(x));
    let (sy, cy) = (libm::sinf(y), libm::cosf(y));
    let (sz, cz) = (libm::sinf(z), libm::cosf(z));
    let mirror = if reflected { -1.0 } else { 1.0 };
    UnitQuaternion::new_normalize(Quaternion::new(
        cx * cy * cz + sx * sy * sz,
        mirror * (sx * cy * cz - cx * sy * sz),
        mirror * (cx * sy * cz + sx * cy * sz),
        cx * cy * sz - sx * sy * cz,
    ))
}
pub fn local_point(size: [f32; 3], normalized: [f32; 3], reflected: bool) -> Point3<f32> {
    Point3::new(
        size[0] * normalized[0] * 0.5,
        size[1] * normalized[1] * 0.5,
        size[2] * normalized[2] * 0.5 * if reflected { -1.0 } else { 1.0 },
    )
}
struct Builder<'a> {
    genome: &'a Genome,
    limits: &'a Limits,
    body: Body,
    mass: f32,
}
impl Builder<'_> {
    fn append(
        &mut self,
        gene: usize,
        parent: Option<usize>,
        edge: Option<&Edge>,
        counts: &mut [u8],
    ) -> Option<usize> {
        let b = &self.genome.bones[gene];
        let depth = parent.map_or(0, |p| self.body.parts[p].depth + 1);
        let scale = parent.map_or(1.0, |p| {
            self.body.parts[p].scale * edge.expect("child edge").scale
        });
        let size = b.size.map(|x| x * scale);
        let mass = b.volume() * b.density * scale.powi(3);
        if self.body.parts.len() >= self.limits.max_parts
            || depth > self.limits.max_depth
            || counts[gene] >= b.recursion_limit
            || size.iter().any(|x| *x < self.limits.min_size)
            || self.mass + mass > self.limits.max_mass
        {
            return None;
        }
        let reflected =
            parent.is_some_and(|p| self.body.parts[p].reflected ^ edge.expect("edge").reflect);
        let pose = if let Some(p) = parent {
            let e = edge.expect("edge");
            let pp = &self.body.parts[p];
            let joint = pp.pose * local_point(pp.size, e.attachment, pp.reflected);
            let rot = pp.pose.rotation * rotation(e.orientation, reflected);
            let offset = rot * local_point(size, e.child_attachment, reflected).coords;
            Isometry3::from_parts(Translation3::from(joint.coords - offset), rot)
        } else {
            Isometry3::identity()
        };
        let i = self.body.parts.len();
        self.mass += mass;
        self.body.parts.push(Part {
            gene,
            parent,
            incoming: edge.cloned(),
            children: vec![None; b.edges.len()],
            pose,
            size,
            scale,
            reflected,
            mass,
            depth,
        });
        counts[gene] += 1;
        for (ei, e) in b.edges.iter().enumerate() {
            if e.terminal_only && counts[gene] < b.recursion_limit {
                continue;
            }
            let child = self.append(e.child, Some(i), Some(e), counts);
            self.body.parts[i].children[ei] = child;
        }
        counts[gene] -= 1;
        Some(i)
    }
}
impl Body {
    pub fn mass(&self) -> f32 {
        self.parts.iter().map(|p| p.mass).sum()
    }
    fn coverage(
        &self,
        a: usize,
        b: usize,
        origin: [f32; 3],
        insertion: [f32; 3],
    ) -> Vec<(usize, usize)> {
        let pa = &self.parts[a];
        let pb = &self.parts[b];
        let start = pa.pose * Point3::from(origin);
        let end = pb.pose * Point3::from(insertion);
        let delta = end - start;
        if delta.norm_squared() < 1e-10 {
            return vec![];
        }
        let force = delta.normalize();
        let mut left = vec![a];
        let mut right = vec![b];
        while let Some(p) = self.parts[*left.last().expect("nonempty")].parent {
            left.push(p);
        }
        while let Some(p) = self.parts[*right.last().expect("nonempty")].parent {
            right.push(p);
        }
        while left.last() == right.last() && !left.is_empty() {
            left.pop();
            right.pop();
        }
        let mut result = vec![];
        for child in left.into_iter().chain(right) {
            let part = &self.parts[child];
            let e = part.incoming.as_ref().expect("nonroot");
            let joint = part.pose * local_point(part.size, e.child_attachment, part.reflected);
            let torque = part.pose.rotation.inverse() * ((start - joint).cross(&force));
            for axis in e.kind.axes() {
                if torque[*axis].abs() > 1e-5 {
                    result.push((child, *axis));
                }
            }
        }
        result.sort();
        result.dedup();
        result
    }
    fn push_muscle(&mut self, mut m: ExpandedMuscle) {
        m.covers = self.coverage(m.origin_part, m.insertion_part, m.origin, m.insertion);
        if !m.covers.is_empty() {
            self.muscles.push(m);
        }
    }
    pub fn actuated_axes_covered(&self) -> bool {
        self.parts.iter().enumerate().skip(1).all(|(i, p)| {
            p.incoming
                .as_ref()
                .expect("incoming")
                .kind
                .axes()
                .iter()
                .all(|axis| self.muscles.iter().any(|m| m.covers.contains(&(i, *axis))))
        })
    }
}
impl Genome {
    pub fn expand(&self, limits: &Limits) -> Result<Body, String> {
        self.validate(limits)?;
        let mut builder = Builder {
            genome: self,
            limits,
            body: Body {
                parts: vec![],
                muscles: vec![],
            },
            mass: 0.0,
        };
        builder
            .append(self.root, None, None, &mut vec![0; self.bones.len()])
            .ok_or("root exceeds body bounds")?;
        let mut body = builder.body;
        for i in 0..body.parts.len() {
            for m in &self.bones[body.parts[i].gene].muscles {
                let mut target = Some(i);
                for step in &m.route {
                    target = target.and_then(|p| match step {
                        Route::Parent => body.parts[p].parent,
                        Route::Child(slot) => body.parts[p].children.get(*slot).copied().flatten(),
                    });
                }
                if let Some(j) = target {
                    let p = &body.parts[i];
                    let q = &body.parts[j];
                    body.push_muscle(ExpandedMuscle {
                        properties: m.properties.clone(),
                        origin_part: i,
                        insertion_part: j,
                        origin: local_point(p.size, m.origin, p.reflected).coords.into(),
                        insertion: local_point(q.size, m.insertion, q.reflected).coords.into(),
                        area: m.area * p.scale * p.scale,
                        output: m.output,
                        covers: vec![],
                    });
                }
            }
        }
        let hash = self.hash();
        let seed = u64::from_le_bytes(hash[..8].try_into().expect("eight bytes"));
        // A geometry-aware genesis pass covers every missing degree of freedom.
        for child in 1..body.parts.len() {
            let parent = body.parts[child].parent.expect("nonroot");
            let axes = body.parts[child]
                .incoming
                .as_ref()
                .expect("edge")
                .kind
                .axes();
            for axis in axes {
                if body
                    .muscles
                    .iter()
                    .any(|m| m.covers.contains(&(child, *axis)))
                {
                    continue;
                }
                let mut rng = crate::rng(seed, 0x4d555343, child as u64, *axis as u64);
                for _ in 0..128 {
                    let mut point = || {
                        let mut v = [
                            rng.gen_range(-1.0..=1.0),
                            rng.gen_range(-1.0..=1.0),
                            rng.gen_range(-1.0..=1.0),
                        ];
                        let k = rng.gen_range(0..3);
                        v[k] = if rng.gen_bool(0.5) { -1.0 } else { 1.0 };
                        v
                    };
                    let p = &body.parts[parent];
                    let q = &body.parts[child];
                    let origin = local_point(p.size, point(), p.reflected).coords.into();
                    let insertion = local_point(q.size, point(), q.reflected).coords.into();
                    let covers = body.coverage(parent, child, origin, insertion);
                    if covers.contains(&(child, *axis)) {
                        body.muscles.push(ExpandedMuscle {
                            properties: MuscleProperties::default(),
                            origin_part: parent,
                            insertion_part: child,
                            origin,
                            insertion,
                            area: 0.1,
                            output: 0,
                            covers,
                        });
                        break;
                    }
                }
            }
        }
        if !body.actuated_axes_covered() {
            return Err("genesis could not produce leverage".into());
        }
        Ok(body)
    }
}
