extern crate alloc;

use alloc::vec::Vec;
use pinocchio::program_error::ProgramError;

use crate::{
    constants::{PROOF_COUNTER_DISCRIMINATOR, PROOF_COUNTER_SIZE},
    error::RedpillVerifierError,
};

/// Program-wide counter of stored proofs. PDA seeds: ["proof_counter"]
#[derive(Clone, Debug, PartialEq)]
pub struct ProofCounter {
    pub count: u64,
}

impl ProofCounter {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(PROOF_COUNTER_SIZE);
        buf.push(PROOF_COUNTER_DISCRIMINATOR);
        buf.extend_from_slice(&self.count.to_le_bytes());
        buf
    }

    pub fn try_from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < PROOF_COUNTER_SIZE {
            return Err(RedpillVerifierError::AccountDataTooSmall.into());
        }
        if data[0] != PROOF_COUNTER_DISCRIMINATOR {
            return Err(RedpillVerifierError::InvalidDiscriminator.into());
        }
        let count = u64::from_le_bytes(data[1..9].try_into().unwrap());
        Ok(Self { count })
    }
}
