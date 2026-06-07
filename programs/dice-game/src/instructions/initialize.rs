use anchor_lang::prelude::{
    prelude::*,
    system_prelude::{transfer, Transfer},
};

#[derive(Accounts)]

pub struct Initialize<'info> {
    #[account(mut)]
    pub house: Signer<'info>,
    #[accounts (
        mut
    )]
    

    pub system_program: Program<'info, System>,
}
