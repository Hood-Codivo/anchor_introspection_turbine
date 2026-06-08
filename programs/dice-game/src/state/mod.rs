use anchor_lang::prelude::*;

pub const HOUSE_EDGE_BASIS_POINTS: u16 = 150;
pub const MIN_BET_LAMPORTS: u64 = 100_000_000; // 0.1 SOL
pub const MIN_ROLL: u8 = 1;
pub const MAX_ROLL: u8 = 99;

#[account]
#[derive(InitSpace)]

pub struct Bet {
    pub player: Pubkey,
    pub seed: u128,
    pub slot: u64,
    pub amount: u64,
    pub roll: u8,
    pub bump: u8,
}

impl Bet  {
    pub fn to_slice(&self) -> Vec<u8> {
        let mut s = self.player.to_bytes().to_vec();
        s.extend_from_slice(&self.seed.to_le_bytes());
        s.extend_from_slice(&self.slot.to_le_bytes());
        s.extend_from_slice(&self.amount.to_le_bytes());
        s.extend_from_slice(&self.roll.to_le_bytes());
        s.extend_from_slice(&self.bump.to_le_bytes());
        s
    }
}