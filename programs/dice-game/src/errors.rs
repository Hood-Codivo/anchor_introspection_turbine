use anchor_lang::prelude::*;

#[error_code]
pub enum DiceError {
    #[msg("Bet amount is below the minimum allowed bet")]
    MinimumBet,
    #[msg("Roll must be greater than or equal to the minimum roll")]
    MinimumRoll,
    #[msg("Roll must be less than or equal to the maximum roll")]
    MaximumRoll,
    #[msg("Operation resulted in an arithmetic overflow")]
    Overflow,
    #[msg("Ed25519 instruction data is shorter than expected")]
    Ed25519DataLength,
    #[msg("Instruction does not target the Ed25519 program")]
    Ed25519Program,
    #[msg("Ed25519 instruction must not reference any accounts")]
    Ed25519Accounts,
    #[msg("Ed25519 instruction must contain exactly one signature")]
    Ed25519SignatureMustBeOne,
    #[msg("Ed25519 signature offsets must reference the current instruction")]
    Ed25519Header,
    #[msg("Could not parse the Ed25519 public key")]
    Ed25519PublicKey,
    #[msg("Ed25519 public key does not match the house")]
    Ed25519Pubkey,
    #[msg("Ed25519 signature does not match the expected signature")]
    Ed25519Signature,
    #[msg("Ed25519 message does not match the expected bet data")]
    Ed25519Message,
}
