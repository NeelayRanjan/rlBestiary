//! Versioned, bounded replay files with lossless run-length compression.
use crate::combine::Replay;
const LIMIT: usize = 128 * 1024 * 1024;
impl Replay {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let raw = postcard::to_allocvec(self).map_err(|e| e.to_string())?;
        if raw.len() > LIMIT {
            return Err("replay too large".into());
        }
        let mut out = b"BSTRPL01".to_vec();
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        let mut i = 0;
        while i < raw.len() {
            let mut run = 1usize;
            while i + run < raw.len() && raw[i + run] == raw[i] && run < 128 {
                run += 1;
            }
            if run >= 3 {
                out.push(0x80 | (run - 1) as u8);
                out.push(raw[i]);
                i += run;
            } else {
                let start = i;
                i += 1;
                while i < raw.len() && i - start < 128 {
                    if i + 2 < raw.len() && raw[i] == raw[i + 1] && raw[i] == raw[i + 2] {
                        break;
                    }
                    i += 1;
                }
                out.push((i - start - 1) as u8);
                out.extend_from_slice(&raw[start..i]);
            }
        }
        out.extend_from_slice(&bestiary_core::digest(&raw).map_err(|e| e.to_string())?);
        Ok(out)
    }
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() < 44 || &data[..8] != b"BSTRPL01" {
            return Err("invalid replay header".into());
        }
        let expected = u32::from_le_bytes(data[8..12].try_into().expect("four bytes")) as usize;
        if expected > LIMIT {
            return Err("replay exceeds size limit".into());
        }
        let payload = &data[12..data.len() - 32];
        let mut raw = Vec::with_capacity(expected);
        let mut i = 0;
        while i < payload.len() {
            let tag = payload[i];
            i += 1;
            let count = usize::from(tag & 127) + 1;
            if raw.len() + count > expected {
                return Err("replay length mismatch".into());
            }
            if tag & 128 != 0 {
                let byte = *payload.get(i).ok_or("truncated run")?;
                i += 1;
                raw.resize(raw.len() + count, byte);
            } else {
                let bytes = payload.get(i..i + count).ok_or("truncated literal")?;
                raw.extend_from_slice(bytes);
                i += count;
            }
        }
        if raw.len() != expected
            || bestiary_core::digest(&raw)
                .map_err(|e| e.to_string())?
                .as_slice()
                != &data[data.len() - 32..]
        {
            return Err("corrupt replay".into());
        }
        let (replay, tail): (Self, &[u8]) =
            postcard::take_from_bytes(&raw).map_err(|e| e.to_string())?;
        if !tail.is_empty() || !replay.dt.is_finite() || replay.dt <= 0.0 {
            return Err("invalid replay timing or tail".into());
        }
        if replay.frames.iter().any(|frame| {
            frame.len() > 33
                || frame.iter().any(|p| {
                    p.translation
                        .iter()
                        .chain(&p.rotation)
                        .any(|x| !x.is_finite())
                })
        }) {
            return Err("invalid replay pose".into());
        }
        Ok(replay)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combine::TrialKind, physics::Pose};
    #[test]
    fn replay_reproduces_recorded_trajectory_and_rejects_corruption() {
        let replay = Replay {
            kind: TrialKind::Sprint,
            dt: 1.0 / 240.0,
            frames: vec![
                vec![
                    Pose {
                        translation: [0.0; 3],
                        rotation: [0.0, 0.0, 0.0, 1.0]
                    };
                    12
                ];
                100
            ],
        };
        let bytes = replay.encode().unwrap();
        assert!(bytes.len() < postcard::to_allocvec(&replay).unwrap().len());
        assert_eq!(Replay::decode(&bytes).unwrap(), replay);
        let mut broken = bytes.clone();
        broken[15] ^= 1;
        assert!(Replay::decode(&broken).is_err());
        for end in [0, 8, 20, bytes.len() - 1] {
            assert!(Replay::decode(&bytes[..end]).is_err());
        }
    }
}
