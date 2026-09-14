use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

/// Assert the account is a signer.
#[inline(always)]
pub fn assert_signer(account: &AccountInfo) -> Result<(), ProgramError> {
    if !account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

/// Assert the account is writable.
#[inline(always)]
pub fn assert_writable(account: &AccountInfo) -> Result<(), ProgramError> {
    if !account.is_writable() {
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

/// Assert account is owned by `expected_owner`.
#[inline(always)]
pub fn assert_owner(account: &AccountInfo, expected_owner: &Pubkey) -> Result<(), ProgramError> {
    if account.owner() != expected_owner {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Assert an account is owned by the System Program.
#[inline(always)]
pub fn assert_system_program_owned(account: &AccountInfo) -> Result<(), ProgramError> {
    // System program key: 11111111111111111111111111111111
    const SYSTEM_PROGRAM_ID: &[u8; 32] = &[0u8; 32];
    if account.owner().as_ref() != SYSTEM_PROGRAM_ID.as_ref() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Assert an account is uninitialized (data is all zeroes or empty).
#[inline(always)]
pub fn assert_uninitialized(account: &AccountInfo) -> Result<(), ProgramError> {
    let data = account.try_borrow_data()?;
    if !data.is_empty() && data[0] != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    Ok(())
}
