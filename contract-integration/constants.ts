import * as anchor from '@coral-xyz/anchor';
import { Buffer } from 'buffer';

// LAZOR.KIT PROGRAM - PDA Seeds
export const SMART_WALLET_SEED = Buffer.from('smart_wallet');
export const SMART_WALLET_CONFIG_SEED = Buffer.from('smart_wallet_config');
export const SMART_WALLET_AUTHENTICATOR_SEED = Buffer.from('smart_wallet_authenticator');
export const WHITELIST_RULE_PROGRAMS_SEED = Buffer.from('whitelist_rule_programs');
export const CONFIG_SEED = Buffer.from('config');
export const AUTHORITY_SEED = Buffer.from('authority');
export const CPI_COMMIT_SEED = Buffer.from('cpi_commit');

// RULE PROGRAM SEEDS
export const RULE_DATA_SEED = Buffer.from('rule_data');
export const MEMBER_SEED = Buffer.from('member');
export const RULE_SEED = Buffer.from('rule');

// ADDRESS LOOKUP TABLE for Versioned Transactions (v0)
// This lookup table contains frequently used program IDs and accounts
// to reduce transaction size and enable more complex operations
export const ADDRESS_LOOKUP_TABLE = new anchor.web3.PublicKey(
  '7Pr3DG7tRPAjVb44gqbxTj1KstikAuVZY7YmXdotVjLA'
);
