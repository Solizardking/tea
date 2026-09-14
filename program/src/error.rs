use pinocchio::program_error::ProgramError;

#[derive(Clone, Debug, PartialEq)]
pub enum RedpillVerifierError {
    // ── V1 errors (stable, kept for backwards compat) ────────────────────────
    /// Account discriminator does not match expected value
    InvalidDiscriminator,
    /// The supplied quote hash is not 32 bytes
    InvalidQuoteHash,
    /// The supplied signing address is not 20 bytes
    InvalidSigningAddress,
    /// A proof for this quote hash already exists
    ProofAlreadyExists,
    /// Account data is too small to deserialize
    AccountDataTooSmall,
    /// PDA derivation mismatch
    InvalidPda,
    /// Arithmetic overflow
    ArithmeticOverflow,

    // ── V2 errors ────────────────────────────────────────────────────────────
    /// The secp256k1 instruction is not at the specified index
    SecpIxNotFound,
    /// The secp256k1 instruction program ID is not the native secp256k1 precompile
    SecpIxWrongProgram,
    /// The secp256k1 instruction is at or after the current instruction index
    SecpIxMustPrecedeCurrentIx,
    /// The message hash inside the secp256k1 instruction does not match the expected payload hash
    SecpMessageHashMismatch,
    /// The recovered Ethereum address in the secp256k1 instruction does not match signing_address
    SecpAddressMismatch,
    /// The canonical signed payload hash supplied does not match the derived payload
    PayloadHashMismatch,
    /// The proof has expired (expires_at is in the past)
    ProofExpired,
    /// The verifier is not in the registry or has been revoked
    VerifierNotAuthorized,
    /// Nullifier already consumed (proof replay detected)
    NullifierAlreadyConsumed,
    /// Proof status does not allow this operation
    InvalidProofStatus,
    /// The sharded day counter tag is not exactly ASCII YYYYMMDD
    InvalidDayTag,
    /// The requested verification level is not supported by this program
    InvalidVerificationLevel,
}

impl From<RedpillVerifierError> for ProgramError {
    fn from(e: RedpillVerifierError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
