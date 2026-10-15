//! Deterministic articulated physics and muscle evaluation. No renderer or async.
pub mod config;
pub mod muscle;
pub mod physics;
pub const PHYSICS_DT: f32 = 1.0 / 240.0;
