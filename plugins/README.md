# Lazorkit V2 Plugins

This directory contains example plugins for Lazorkit V2. Each plugin is a separate Solana program that implements permission checking logic.

## Plugin Architecture

Each plugin must implement:
1. **CheckPermission** instruction - Called by Lazorkit V2 to check if an operation is allowed
2. **Config Account** - A PDA account that stores plugin-specific configuration

## Plugin Interface

### CheckPermission Instruction

**CPI Call Format:**
```
Instruction Data:
  [0] - PluginInstruction::CheckPermission (u8)
  [1-2] - instruction_data_len (u16, little-endian)
  [3..] - instruction_data (raw instruction bytes to check)

Accounts:
  [0] - Plugin Config PDA (writable)
  [1] - Smart Wallet PDA (signer)
  [2] - WalletState (read-only, for context)
  [3..] - Additional accounts from the instruction being checked
```

**Expected Behavior:**
- Return `Ok(())` if the operation is allowed
- Return `Err(ProgramError)` if the operation is denied

## Example Plugins

### 1. Sol Limit Plugin (`sol-limit/`)

Enforces a maximum SOL transfer limit per authority.

**Config Structure:**
```rust
pub struct SolLimitConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_state: Pubkey,
    pub remaining_amount: u64,  // Remaining SOL limit in lamports
}
```

**Features:**
- Tracks remaining SOL that can be transferred
- Decreases limit as operations are performed
- Blocks transfers that would exceed the limit

### 2. Program Whitelist Plugin (`program-whitelist/`)

Allows interactions only with whitelisted programs.

**Config Structure:**
```rust
pub struct ProgramWhitelistConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_state: Pubkey,
    pub num_programs: u16,
    // Followed by: program_ids (num_programs * 32 bytes)
}
```

**Features:**
- Maintains a list of whitelisted program IDs
- Checks each instruction's program_id against the whitelist
- Blocks interactions with non-whitelisted programs

### 3. All Permission Plugin (`all-permission/`)

Simple plugin that allows all operations (useful for testing).

**Config Structure:**
```rust
pub struct AllPermissionConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_state: Pubkey,
}
```

**Features:**
- Always returns `Ok(())` for CheckPermission
- Useful for testing or unrestricted authorities

## Building Plugins

```bash
# Build a plugin
cd plugins/sol-limit
cargo build-sbf

# Deploy
solana program deploy target/deploy/lazorkit_sol_limit_plugin.so
```

## Usage Example

1. **Create Plugin Config PDA:**
```rust
let (config_pda, bump) = Pubkey::find_program_address(
    &[
        b"sol_limit_config",
        wallet_state.as_ref(),
    ],
    &plugin_program_id,
);
```

2. **Initialize Config:**
```rust
// Create and initialize config account with initial limit
let config = SolLimitConfig {
    discriminator: Discriminator::WalletState as u8,
    bump,
    wallet_state: *wallet_state,
    remaining_amount: 1_000_000_000, // 1 SOL
};
```

3. **Add Plugin to Wallet:**
```rust
// Use Lazorkit V2 AddPlugin instruction
// Pass plugin_program_id and config_pda
```

4. **Plugin will be called automatically during SignV2**

## Creating Custom Plugins

To create a new plugin:

1. Create a new directory under `plugins/`
2. Add `Cargo.toml` with dependencies:
   - `pinocchio`
   - `lazorkit-v2-interface`
   - `lazorkit-v2-state`
   - `lazorkit-v2-assertions`
3. Implement `CheckPermission` instruction handler
4. Define your config account structure
5. Build and deploy

## Notes

- Plugins are called via CPI from Lazorkit V2
- Each plugin must validate that it's being called by Lazorkit V2 (check signer)
- Config accounts should be PDAs derived from wallet_state
- Plugins can update their config via `UpdateConfig` instruction (optional)
