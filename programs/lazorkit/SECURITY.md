# LazorKit Security Documentation

## Overview

LazorKit implements multiple layers of security to protect smart wallets and ensure safe operation of the protocol. This document outlines all security features and best practices.

## Security Architecture

### 1. Input Validation

All inputs are thoroughly validated before processing:

- **Parameter Bounds**: All numeric inputs are checked for overflow/underflow
- **Size Limits**: Enforced maximum sizes for all variable-length data
- **Type Safety**: Strict type checking for all parameters
- **Format Validation**: Passkey format validation (compressed public key)

```rust
// Example validation
validation::validate_credential_id(&credential_id)?;
validation::validate_rule_data(&rule_data)?;
validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
```

### 2. Access Control

Multiple layers of access control protect the system:

- **PDA Verification**: All PDAs are verified with correct seeds and bumps
- **Authority Checks**: Admin operations require proper authority
- **Whitelist Control**: Only whitelisted rule programs can be used
- **Ownership Validation**: Account ownership is verified before operations

### 3. State Management

Robust state management prevents common vulnerabilities:

- **Atomic Operations**: All state changes are atomic
- **Nonce Management**: Replay attack prevention through nonce tracking
- **Overflow Protection**: Checked arithmetic for all calculations
- **State Consistency**: Constraints ensure valid state transitions

### 4. Emergency Controls

The system includes emergency mechanisms:

- **Program Pause**: Authority can pause all operations
- **Emergency Shutdown**: Critical errors trigger protective mode
- **Recovery Options**: Safe recovery paths for error scenarios

### 5. Fee Management

Transparent and secure fee handling:

- **Creation Fees**: Optional fees for wallet creation
- **Execution Fees**: Optional fees for transaction execution
- **Balance Checks**: Ensures sufficient balance before fee deduction
- **Rent Exemption**: Maintains minimum balance for rent

### 6. Event System

Comprehensive event emission for monitoring:

- **Wallet Creation Events**: Track all new wallets
- **Transaction Events**: Monitor all executions
- **Security Events**: Alert on suspicious activities
- **Error Events**: Log handled errors for analysis

## Security Features by Component

### Create Smart Wallet

- Validates passkey format (0x02 or 0x03 prefix)
- Checks credential ID size (max 256 bytes)
- Validates rule data size (max 1024 bytes)
- Ensures sequence number doesn't overflow
- Verifies default rule program is executable
- Checks whitelist membership

### Execute Transaction

- Verifies passkey signature through Secp256r1
- Validates message timestamp (max 5 minutes old)
- Checks nonce to prevent replay attacks
- Ensures rule program matches configuration
- Validates split index for account separation
- Prevents reentrancy attacks
- Checks sufficient balance for operations

### Change Rule Program

- Ensures both programs are whitelisted
- Validates destroy/init discriminators
- Enforces default rule constraint
- Prevents changing to same program
- Atomic rule program swap

### Security Constants

```rust
pub const MAX_CREDENTIAL_ID_SIZE: usize = 256;
pub const MAX_RULE_DATA_SIZE: usize = 1024;
pub const MAX_CPI_DATA_SIZE: usize = 1024;
pub const MAX_REMAINING_ACCOUNTS: usize = 32;
pub const MIN_RENT_EXEMPT_BUFFER: u64 = 1_000_000;
pub const MAX_TRANSACTION_AGE: i64 = 300;
```

## Error Handling

The system includes comprehensive error codes for all failure scenarios:

- Authentication errors
- Validation errors
- State errors
- Security errors
- System errors

Each error provides clear information for debugging while avoiding information leakage.

## Best Practices

1. **Always validate inputs** - Never trust external data
2. **Check account ownership** - Verify PDAs and account owners
3. **Use checked arithmetic** - Prevent overflow/underflow
4. **Emit events** - Enable monitoring and debugging
5. **Handle errors gracefully** - Provide clear error messages
6. **Maintain audit trail** - Log all critical operations

## Threat Model

The system protects against:

- **Replay Attacks**: Through nonce management
- **Signature Forgery**: Using Secp256r1 verification
- **Unauthorized Access**: Through passkey authentication
- **Reentrancy**: By checking program IDs
- **Integer Overflow**: Using checked arithmetic
- **DoS Attacks**: Through size limits and validation
- **Malicious Programs**: Through whitelist control

## Monitoring and Alerts

Monitor these events for security:

- Failed authentication attempts
- Invalid signature verifications
- Rejected transactions
- Unexpected program pauses
- Large value transfers
- Rapid transaction sequences

## Emergency Procedures

In case of security incidents:

1. **Pause Program**: Authority can pause all operations
2. **Investigate**: Use event logs to understand the issue
3. **Patch**: Deploy fixes if vulnerabilities found
4. **Resume**: Unpause program after verification

## Audit Recommendations

Regular audits should focus on:

- Input validation completeness
- Access control effectiveness
- State transition safety
- Error handling coverage
- Event emission accuracy
- Integration test coverage

## Contact

For security concerns or bug reports, please contact the development team through official channels. 