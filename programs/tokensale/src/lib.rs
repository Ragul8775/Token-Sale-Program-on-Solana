use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, Transfer},
};
use std::cmp::min;

declare_id!("72nFkwFd2qVVYAskDJLTvYSZcR7tfrPiactKSv4stVSd");

#[program]
pub mod tokensale {

    use anchor_lang::solana_program::native_token::LAMPORTS_PER_SOL;

    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        token_price: u64,
        purchase_limit: u64,
        admin_pubkey: Pubkey,
    ) -> Result<()> {
        ctx.accounts.config_account.bump = ctx.bumps.config_account;
        ctx.accounts.config_account.token_price = token_price;
        ctx.accounts.config_account.purchase_limit = purchase_limit;
        ctx.accounts.config_account.token_mint = ctx.accounts.token_mint.key();
        ctx.accounts.config_account.admin_pubkey = admin_pubkey;
        Ok(())
    }

    pub fn change_price(ctx: Context<ChangePrice>, new_price: u64) -> Result<()> {
        ctx.accounts.config_account.token_price = new_price;
        Ok(())
    }

    pub fn change_limit(ctx: Context<ChangeLimit>, new_limit: u64) -> Result<()> {
        ctx.accounts.config_account.purchase_limit = new_limit;
        Ok(())
    }

    pub fn add_to_whitelist(ctx: Context<AddToWhitelist>, _pubkey_to_add: Pubkey) -> Result<()> {
        ctx.accounts.user_account.whitelisted = true;
        Ok(())
    }

    pub fn remove_from_whitelist(
        ctx: Context<RemoveFromWhitelist>,
        _pubkey_to_remove: Pubkey,
    ) -> Result<()> {
        ctx.accounts.user_account.whitelisted = false;
        Ok(())
    }

    pub fn buy_token(ctx: Context<BuyToken>, amount: u64) -> Result<()> {
        // Check whitelisted
        assert!(
            ctx.accounts.user_account.whitelisted,
            "You're not whitelisted"
        );
        msg!("You are whitelisted");

        // Check he's not over the limit
        assert!(
            ctx.accounts.user_account.amount_purchased < ctx.accounts.config_account.purchase_limit,
            "You're over the limit"
        );

        let left_to_buy =
            ctx.accounts.config_account.purchase_limit - ctx.accounts.user_account.amount_purchased;
        let amount_buying = min(left_to_buy, amount);

        let amount_sol =
            (amount_buying * ctx.accounts.config_account.token_price) / LAMPORTS_PER_SOL;

        // Receive sol payment for this amount
        msg!("Now sending sol payment");
        let cpi_accounts = anchor_lang::system_program::Transfer {
            from: ctx.accounts.signer.to_account_info().clone(),
            to: ctx
                .accounts
                .token_account_owner_pda
                .to_account_info()
                .clone(),
        };

        let cpi_program = ctx.accounts.system_program.clone();
        let cpi_ctx = CpiContext::new(cpi_program.to_account_info(), cpi_accounts);
        let _ = anchor_lang::system_program::transfer(cpi_ctx, amount_sol);

        msg!("Now sending spl payment");
        // Below is the actual instruction that we are going to send to the Token program.
        let transfer_instruction = Transfer {
            from: ctx.accounts.program_token_account.to_account_info(),
            to: ctx.accounts.user_ata.to_account_info(),
            authority: ctx.accounts.token_account_owner_pda.to_account_info(),
        };
        let bump = ctx.bumps.token_account_owner_pda;
        let seeds = &[b"token_account_owner_pda".as_ref(), &[bump]];
        let signer = &[&seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
            signer,
        );

        let _ = anchor_spl::token::transfer(cpi_ctx, amount_buying)?;

        ctx.accounts.user_account.amount_purchased =
            ctx.accounts.user_account.amount_purchased + amount_buying;

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let transfer_instruction = Transfer {
            from: ctx.accounts.signer_ata.to_account_info(),
            to: ctx.accounts.program_token_account.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        **ctx
            .accounts
            .signer
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;
        **ctx
            .accounts
            .token_account_owner_pda
            .to_account_info()
            .try_borrow_mut_lamports()? -= amount;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = signer, space = 8 + ConfigurationAccount::MAX_SIZE, seeds = [b"CONFIG_ACCOUNT"], bump)]
    pub config_account: Account<'info, ConfigurationAccount>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub token_mint: Account<'info, Mint>,

    /// CHECK:
    #[account(init, payer = signer, space = 8, seeds=[b"token_account_owner_pda"],bump )]
    pub token_account_owner_pda: UncheckedAccount<'info>,

    #[account(init, payer = signer, seeds=[b"PROGRAM_TOKEN_ACCOUNT"],  bump, token::mint=token_mint,  token::authority=token_account_owner_pda)]
    pub program_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, address = config_account.admin_pubkey)]
    pub signer: Signer<'info>,

    #[account(seeds = [b"CONFIG_ACCOUNT"], bump)]
    pub config_account: Account<'info, ConfigurationAccount>,

    #[account(mut, associated_token::mint = token_mint, associated_token::authority = signer, associated_token::token_program = token_program)]
    pub signer_ata: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,

    pub token_mint: Account<'info, Mint>,

    /// CHECK:
    #[account(seeds=[b"token_account_owner_pda"],bump )]
    pub token_account_owner_pda: UncheckedAccount<'info>,

    #[account(mut, seeds=[b"PROGRAM_TOKEN_ACCOUNT"],  bump, token::mint=token_mint,  token::authority=token_account_owner_pda)]
    pub program_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, address = config_account.admin_pubkey)]
    pub signer: Signer<'info>,

    #[account(seeds = [b"CONFIG_ACCOUNT"], bump)]
    pub config_account: Account<'info, ConfigurationAccount>,

    /// CHECK:
    #[account(mut, seeds=[b"token_account_owner_pda"],bump )]
    pub token_account_owner_pda: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChangeLimit<'info> {
    #[account(mut, seeds = [b"CONFIG_ACCOUNT"], bump = config_account.bump)]
    pub config_account: Account<'info, ConfigurationAccount>,
    #[account(mut, address = config_account.admin_pubkey)]
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ChangePrice<'info> {
    #[account(mut, seeds = [b"CONFIG_ACCOUNT"], bump = config_account.bump)]
    pub config_account: Account<'info, ConfigurationAccount>,
    #[account(mut, address = config_account.admin_pubkey)]
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(pubkey_to_add: Pubkey)]
pub struct AddToWhitelist<'info> {
    #[account(init_if_needed, payer = signer, space = 8 + UserAccount::MAX_SIZE, seeds = [b"USER_ACCOUNT", pubkey_to_add.as_ref()], bump)]
    pub user_account: Account<'info, UserAccount>,

    #[account(mut, address = config_account.admin_pubkey)]
    pub signer: Signer<'info>,

    #[account(seeds = [b"CONFIG_ACCOUNT"], bump)]
    pub config_account: Account<'info, ConfigurationAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(pubkey_to_remove: Pubkey)]
pub struct RemoveFromWhitelist<'info> {
    #[account(mut, seeds = [b"USER_ACCOUNT", pubkey_to_remove.as_ref()], bump)]
    pub user_account: Account<'info, UserAccount>,

    #[account(seeds = [b"CONFIG_ACCOUNT"], bump)]
    pub config_account: Account<'info, ConfigurationAccount>,

    #[account(mut, address = config_account.admin_pubkey)]
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    // User pda
    #[account(mut, seeds = [b"USER_ACCOUNT", signer.key().as_ref()], bump)]
    pub user_account: Account<'info, UserAccount>,

    // User ATA
    #[account(init_if_needed, payer = signer, associated_token::mint = token_mint, associated_token::authority = signer, associated_token::token_program = token_program)]
    pub user_ata: Account<'info, TokenAccount>,

    #[account(seeds = [b"CONFIG_ACCOUNT"], bump = config_account.bump)]
    pub config_account: Account<'info, ConfigurationAccount>,

    #[account(address = config_account.token_mint)]
    pub token_mint: Account<'info, Mint>,

    /// CHECK:
    #[account(mut, seeds=[b"token_account_owner_pda"], bump)]
    pub token_account_owner_pda: UncheckedAccount<'info>,

    #[account(mut, seeds=[b"PROGRAM_TOKEN_ACCOUNT"],bump, token::mint=token_mint,  token::authority=token_account_owner_pda)]
    pub program_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[account]
pub struct ConfigurationAccount {
    bump: u8,
    admin_pubkey: Pubkey,
    token_price: u64, // in SOL
    purchase_limit: u64,
    token_mint: Pubkey,
}

impl ConfigurationAccount {
    pub const MAX_SIZE: usize = 8 + 1 + 8 + 32 + 32;
}

#[account]
pub struct UserAccount {
    whitelisted: bool,
    amount_purchased: u64,
}

impl UserAccount {
    pub const MAX_SIZE: usize = 1 + 8;
}
