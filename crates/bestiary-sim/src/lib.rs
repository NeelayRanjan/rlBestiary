//! Deterministic articulated physics and muscle evaluation. No renderer or async.
pub mod config;
pub mod muscle;
pub mod physics;
/// Evaluator semantics: rest-frame proprioception and terrain-load sensing.
pub const SIMULATION_REVISION: u32 = 3;
pub const PHYSICS_DT: f32 = 1.0 / 240.0;

pub mod genome;

pub mod combine;
pub mod replay;

pub mod locomotion;
pub mod training;
