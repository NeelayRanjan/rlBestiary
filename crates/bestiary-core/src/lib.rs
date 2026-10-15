//! Deterministic primitives shared by simulation layers. No rendering or async.
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Serialize;

/// Independent stream for an explicit (world, subsystem, entity, tick) tuple.
pub fn rng(world: u64, subsystem: u64, entity: u64, tick: u64) -> ChaCha8Rng {
    let mut seed = [0; 32];
    for (chunk, value) in seed
        .chunks_exact_mut(8)
        .zip([world, subsystem, entity, tick])
    {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    ChaCha8Rng::from_seed(seed)
}

/// Canonical digest; callers must use ordered collections in serialized state.
pub fn digest<T: Serialize>(value: &T) -> Result<[u8; 32], postcard::Error> {
    Ok(*blake3::hash(&postcard::to_allocvec(value)?).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    #[test]
    fn seeded_tuple_streams_are_repeatable_and_separated() {
        let sample = |args: [u64; 4]| {
            let mut r = rng(args[0], args[1], args[2], args[3]);
            [r.next_u64(), r.next_u64(), r.next_u64()]
        };
        let baseline = sample([1, 2, 3, 4]);
        assert_eq!(baseline, sample([1, 2, 3, 4]));
        for i in 0..4 {
            let mut args = [1, 2, 3, 4];
            args[i] += 1;
            assert_ne!(baseline, sample(args));
        }
    }
    #[test]
    fn canonical_digest_detects_state_changes() {
        assert_eq!(digest(&[1u64, 2]).unwrap(), digest(&[1u64, 2]).unwrap());
        assert_ne!(digest(&[1u64, 2]).unwrap(), digest(&[2u64, 1]).unwrap());
    }
}

pub mod brain;
pub mod genome;
