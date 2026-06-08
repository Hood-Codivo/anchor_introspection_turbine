use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

use solana_ed25519_program::{
    Ed25519SignatureOffsets, PUBKEY_SERIALIZED_SIZE, SIGNATURE_OFFSETS_SERIALIZED_SIZE,
    SIGNATURE_OFFSETS_START, SIGNATURE_SERIALIZED_SIZE,
};

use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_sha256_hasher::hash;

use crate::{
    errors::DiceError,
    state::{Bet, HOUSE_EDGE_BASIS_POINTS},
};

#[derive(Accounts)]
pub struct ResolveBet<'info> {
    #[account(mut)]
    pub house: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"vault", house.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,
    #[account(
        mut,
        has_one = player,
        close = player,
        seeds = [b"bet", vault.key().as_ref(), player.key().as_ref(), &bet.seed.to_le_bytes().as_ref()],
        bump = bet.bump
    )]
    pub bet: Account<'info, Bet>,
    #[account(
        address = solana_sdk_ids::sysvar::instructions::ID
    )]
    pub instruction_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

struct ED25519InstructionData<'a> {
    pub public_key: &'a [u8],
    pub signature: &'a [u8],
    pub message: &'a [u8],
}

fn read_ed25519_instruction_data(data: &[u8], offset: u16, size: u16) -> Result<&[u8]> {
    let start = usize::from(offset);
    let end = start.checked_add(usize::from(size)).ok_or(DiceError::Overflow)?;

    data.get(start..end)
        .ok_or(DiceError::Ed25519DataLength.into())
}

fn deserialize_ed25519_instruction_data(data: &[u8]) -> Result<ED25519InstructionData> {
    require_eq!(data[0], 1u8, DiceError::Ed25519SignatureMustBeOne);

    let offsets_start = SIGNATURE_OFFSETS_START;
    let offsets_end = offsets_start
        .checked_add(SIGNATURE_OFFSETS_SERIALIZED_SIZE)
        .ok_or(DiceError::Overflow)?;
    let offsets_data = data
        .get(offsets_start..offsets_end)
        .ok_or(DiceError::Ed25519DataLength)?;

    let offsets = Ed25519SignatureOffsets {
        signature_offset: u16::from_le_bytes([offsets_data[0], offsets_data[1]]),
        signature_instruction_index: u16::from_le_bytes([offsets_data[2], offsets_data[3]]),
        public_key_offset: u16::from_le_bytes([offsets_data[4], offsets_data[5]]),
        public_key_instruction_index: u16::from_le_bytes([offsets_data[6], offsets_data[7]]),
        message_data_offset: u16::from_le_bytes([offsets_data[8], offsets_data[9]]),
        message_data_size: u16::from_le_bytes([offsets_data[10], offsets_data[11]]),
        message_instruction_index: u16::from_le_bytes([offsets_data[12], offsets_data[13]]),
    };

    require!(
        offsets.signature_instruction_index == u16::MAX
            && offsets.public_key_instruction_index == u16::MAX
            && offsets.message_instruction_index == u16::MAX,
        DiceError::Ed25519Header
    );

    let public_key = read_ed25519_instruction_data(
        data,
        offsets.public_key_offset,
        PUBKEY_SERIALIZED_SIZE as u16,
    )?;

    let signature = read_ed25519_instruction_data(
        data,
        offsets.signature_offset,
        SIGNATURE_SERIALIZED_SIZE as u16,
    )?;

    let message = read_ed25519_instruction_data(
        data,
        offsets.message_data_offset,
        offsets.message_data_size,
    )?;

    Ok(ED25519InstructionData {
        public_key,
        signature,
        message,
    })
}

impl<'info> ResolveBet<'info> {
    pub fn verify_ed25519_signature(&self, sig: &[u8]) -> Result<()> {
        let instruction_sysvar = self.instruction_sysvar.to_account_info();

        let current_index = load_current_index_checked(&instruction_sysvar)
            .map_err(|_| DiceError::Ed25519Signature)?;

        require!(current_index > 0, DiceError::Ed25519Signature);

        let ix = load_instruction_at_checked(usize::from(current_index) - 1, &instruction_sysvar)
            .map_err(|_| DiceError::Ed25519Signature)?;

        require_keys_eq!(
            ix.program_id,
            solana_sdk_ids::ed25519_program::ID,
            DiceError::Ed25519Program
        );

        require_eq!(ix.accounts.len(), 0, DiceError::Ed25519Accounts);

        let ed25519_data = deserialize_ed25519_instruction_data(&ix.data)?;

        require_keys_eq!(
            Pubkey::try_from(ed25519_data.public_key).map_err(|_| DiceError::Ed25519PublicKey)?,
            self.house.key(),
            DiceError::Ed25519Pubkey
        );

        require!(ed25519_data.signature == sig, DiceError::Ed25519Signature);

        let expected_message = self.bet.to_slice();

        if ed25519_data.message != expected_message.as_slice() {
            return Err(DiceError::Ed25519Message.into());
        }

        Ok(())
    }

    pub fn resolve_bet(&mut self, bumps: &ResolveBetBumps, sig: &[u8]) -> Result<()> {
        let hash = hash(sig).to_bytes();
        let mut hash_16: [u8; 16] = [0; 16];

        hash_16.copy_from_slice(&hash[0..16]);
        let lower = u128::from_le_bytes(hash_16);
        hash_16.copy_from_slice(&hash[16..32]);
        let upper = u128::from_le_bytes(hash_16);

        let roll = lower.wrapping_add(upper).wrapping_rem(100) as u8 + 1;

        if self.bet.roll > roll {
            let winning_numbers = self.bet.roll as u128 - 1;

            let payout = (self.bet.amount as u128)
                .checked_mul(10_000 - HOUSE_EDGE_BASIS_POINTS as u128)
                .ok_or(DiceError::Overflow)?
                .checked_div(winning_numbers)
                .ok_or(DiceError::Overflow)?
                .checked_div(100)
                .ok_or(DiceError::Overflow)?;

            let payout = u64::try_from(payout).map_err(|_| DiceError::Overflow)?;

            let house_key = self.house.key();
            let signer_seeds: &[&[&[u8]]] = &[&[
                b"vault",
                house_key.as_ref(),
                &[bumps.vault],
            ]];

            let accounts = Transfer {
                from: self.vault.to_account_info(),
                to: self.player.to_account_info(),
            };

            let ctx = CpiContext::new_with_signer(
                self.system_program.to_account_info(),
                accounts,
                signer_seeds,
            );

            transfer(ctx, payout)?;
        }

        Ok(())
    }
}
