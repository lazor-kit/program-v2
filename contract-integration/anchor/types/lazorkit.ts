/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/lazorkit.json`.
 */
export type Lazorkit = {
  "address": "Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh",
  "metadata": {
    "name": "lazorkit",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "Created with Anchor"
  },
  "docs": [
    "LazorKit: Smart Wallet with WebAuthn Passkey Authentication"
  ],
  "instructions": [
    {
      "name": "addPolicyProgram",
      "discriminator": [
        172,
        91,
        65,
        142,
        231,
        42,
        251,
        227
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true,
          "relations": [
            "config"
          ]
        },
        {
          "name": "config",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "policyProgramRegistry",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121,
                  95,
                  114,
                  101,
                  103,
                  105,
                  115,
                  116,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "newPolicyProgram"
        }
      ],
      "args": []
    },
    {
      "name": "createSmartWallet",
      "discriminator": [
        129,
        39,
        235,
        18,
        132,
        68,
        203,
        19
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "The account that pays for the wallet creation and initial SOL transfer"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "policyProgramRegistry",
          "docs": [
            "Policy program registry that validates the default policy program"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121,
                  95,
                  114,
                  101,
                  103,
                  105,
                  115,
                  116,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "smartWallet",
          "docs": [
            "The smart wallet address PDA being created with the provided wallet ID"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  109,
                  97,
                  114,
                  116,
                  95,
                  119,
                  97,
                  108,
                  108,
                  101,
                  116
                ]
              },
              {
                "kind": "arg",
                "path": "args.wallet_id"
              }
            ]
          }
        },
        {
          "name": "smartWalletState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  119,
                  97,
                  108,
                  108,
                  101,
                  116,
                  95,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              },
              {
                "kind": "arg",
                "path": "args.wallet_id"
              }
            ]
          }
        },
        {
          "name": "config",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "defaultPolicyProgram"
        },
        {
          "name": "systemProgram",
          "docs": [
            "System program for account creation and SOL transfers"
          ],
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "createSmartWalletArgs"
            }
          }
        }
      ]
    },
    {
      "name": "initializeProgram",
      "discriminator": [
        176,
        107,
        205,
        168,
        24,
        157,
        175,
        103
      ],
      "accounts": [
        {
          "name": "signer",
          "docs": [
            "The signer of the transaction, who will be the initial authority."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "config",
          "docs": [
            "The program's configuration account."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "policyProgramRegistry",
          "docs": [
            "The registry of policy programs that can be used with smart wallets."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121,
                  95,
                  114,
                  101,
                  103,
                  105,
                  115,
                  116,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "defaultPolicyProgram",
          "docs": [
            "The default policy program to be used for new smart wallets."
          ]
        },
        {
          "name": "systemProgram",
          "docs": [
            "The system program."
          ],
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "manageVault",
      "discriminator": [
        165,
        7,
        106,
        242,
        73,
        193,
        195,
        128
      ],
      "accounts": [
        {
          "name": "authority",
          "docs": [
            "The current authority of the program."
          ],
          "writable": true,
          "signer": true,
          "relations": [
            "config"
          ]
        },
        {
          "name": "config",
          "docs": [
            "The program's configuration account."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "vault",
          "docs": [
            "Individual vault PDA (empty account that holds SOL)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  97,
                  122,
                  111,
                  114,
                  107,
                  105,
                  116,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "arg",
                "path": "index"
              }
            ]
          }
        },
        {
          "name": "destination",
          "writable": true
        },
        {
          "name": "systemProgram",
          "docs": [
            "System program"
          ],
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "action",
          "type": "u8"
        },
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "index",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateConfig",
      "discriminator": [
        29,
        158,
        252,
        191,
        10,
        83,
        219,
        99
      ],
      "accounts": [
        {
          "name": "authority",
          "docs": [
            "The current authority of the program."
          ],
          "writable": true,
          "signer": true,
          "relations": [
            "config"
          ]
        },
        {
          "name": "config",
          "docs": [
            "The program's configuration account."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "param",
          "type": {
            "defined": {
              "name": "updateType"
            }
          }
        },
        {
          "name": "value",
          "type": "u64"
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "config",
      "discriminator": [
        155,
        12,
        170,
        224,
        30,
        250,
        204,
        130
      ]
    },
    {
      "name": "policyProgramRegistry",
      "discriminator": [
        158,
        67,
        114,
        157,
        27,
        153,
        86,
        72
      ]
    },
    {
      "name": "walletState",
      "discriminator": [
        126,
        186,
        0,
        158,
        92,
        223,
        167,
        68
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "passkeyMismatch",
      "msg": "Passkey public key mismatch with stored authenticator"
    },
    {
      "code": 6001,
      "name": "smartWalletConfigMismatch",
      "msg": "Smart wallet address mismatch with authenticator"
    },
    {
      "code": 6002,
      "name": "secp256r1InvalidLength",
      "msg": "Secp256r1 instruction has invalid data length"
    },
    {
      "code": 6003,
      "name": "secp256r1HeaderMismatch",
      "msg": "Secp256r1 instruction header validation failed"
    },
    {
      "code": 6004,
      "name": "secp256r1DataMismatch",
      "msg": "Secp256r1 signature data validation failed"
    },
    {
      "code": 6005,
      "name": "invalidSignature",
      "msg": "Invalid signature provided for passkey verification"
    },
    {
      "code": 6006,
      "name": "clientDataInvalidUtf8",
      "msg": "Client data JSON is not valid UTF-8"
    },
    {
      "code": 6007,
      "name": "clientDataJsonParseError",
      "msg": "Client data JSON parsing failed"
    },
    {
      "code": 6008,
      "name": "challengeMissing",
      "msg": "Challenge field missing from client data JSON"
    },
    {
      "code": 6009,
      "name": "challengeBase64DecodeError",
      "msg": "Challenge base64 decoding failed"
    },
    {
      "code": 6010,
      "name": "challengeDeserializationError",
      "msg": "Challenge message deserialization failed"
    },
    {
      "code": 6011,
      "name": "timestampTooOld",
      "msg": "Message timestamp is too far in the past"
    },
    {
      "code": 6012,
      "name": "timestampTooNew",
      "msg": "Message timestamp is too far in the future"
    },
    {
      "code": 6013,
      "name": "nonceMismatch",
      "msg": "Nonce mismatch: expected different value"
    },
    {
      "code": 6014,
      "name": "nonceOverflow",
      "msg": "Nonce overflow: cannot increment further"
    },
    {
      "code": 6015,
      "name": "hashMismatch",
      "msg": "Message hash mismatch: expected different value"
    },
    {
      "code": 6016,
      "name": "policyProgramNotRegistered",
      "msg": "Policy program not found in registry"
    },
    {
      "code": 6017,
      "name": "whitelistFull",
      "msg": "The policy program registry is full."
    },
    {
      "code": 6018,
      "name": "invalidCheckPolicyDiscriminator",
      "msg": "Invalid instruction discriminator for check_policy"
    },
    {
      "code": 6019,
      "name": "invalidDestroyDiscriminator",
      "msg": "Invalid instruction discriminator for destroy"
    },
    {
      "code": 6020,
      "name": "invalidInitPolicyDiscriminator",
      "msg": "Invalid instruction discriminator for init_policy"
    },
    {
      "code": 6021,
      "name": "policyProgramsIdentical",
      "msg": "Old and new policy programs are identical"
    },
    {
      "code": 6022,
      "name": "noDefaultPolicyProgram",
      "msg": "Neither old nor new policy program is the default"
    },
    {
      "code": 6023,
      "name": "policyProgramAlreadyRegistered",
      "msg": "Policy program already registered"
    },
    {
      "code": 6024,
      "name": "invalidRemainingAccounts",
      "msg": "Invalid remaining accounts"
    },
    {
      "code": 6025,
      "name": "cpiDataMissing",
      "msg": "CPI data is required but not provided"
    },
    {
      "code": 6026,
      "name": "insufficientPolicyAccounts",
      "msg": "Insufficient remaining accounts for policy instruction"
    },
    {
      "code": 6027,
      "name": "insufficientCpiAccounts",
      "msg": "Insufficient remaining accounts for CPI instruction"
    },
    {
      "code": 6028,
      "name": "accountSliceOutOfBounds",
      "msg": "Account slice index out of bounds"
    },
    {
      "code": 6029,
      "name": "transferAmountOverflow",
      "msg": "Transfer amount would cause arithmetic overflow"
    },
    {
      "code": 6030,
      "name": "invalidBumpSeed",
      "msg": "Invalid bump seed for PDA derivation"
    },
    {
      "code": 6031,
      "name": "invalidAccountOwner",
      "msg": "Account owner verification failed"
    },
    {
      "code": 6032,
      "name": "programNotExecutable",
      "msg": "Program not executable"
    },
    {
      "code": 6033,
      "name": "programPaused",
      "msg": "Program is paused"
    },
    {
      "code": 6034,
      "name": "walletDeviceAlreadyInitialized",
      "msg": "Wallet device already initialized"
    },
    {
      "code": 6035,
      "name": "credentialIdTooLarge",
      "msg": "Credential ID exceeds maximum allowed size"
    },
    {
      "code": 6036,
      "name": "credentialIdEmpty",
      "msg": "Credential ID cannot be empty"
    },
    {
      "code": 6037,
      "name": "policyDataTooLarge",
      "msg": "Policy data exceeds maximum allowed size"
    },
    {
      "code": 6038,
      "name": "cpiDataTooLarge",
      "msg": "CPI data exceeds maximum allowed size"
    },
    {
      "code": 6039,
      "name": "tooManyRemainingAccounts",
      "msg": "Too many remaining accounts provided"
    },
    {
      "code": 6040,
      "name": "invalidPdaDerivation",
      "msg": "Invalid PDA derivation"
    },
    {
      "code": 6041,
      "name": "transactionTooOld",
      "msg": "Transaction is too old"
    },
    {
      "code": 6042,
      "name": "invalidAccountData",
      "msg": "Invalid account data"
    },
    {
      "code": 6043,
      "name": "invalidInstructionData",
      "msg": "Invalid instruction data"
    },
    {
      "code": 6044,
      "name": "accountAlreadyInitialized",
      "msg": "Account already initialized"
    },
    {
      "code": 6045,
      "name": "invalidAccountState",
      "msg": "Invalid account state"
    },
    {
      "code": 6046,
      "name": "invalidFeeAmount",
      "msg": "Invalid fee amount"
    },
    {
      "code": 6047,
      "name": "insufficientBalanceForFee",
      "msg": "Insufficient balance for fee"
    },
    {
      "code": 6048,
      "name": "invalidAuthority",
      "msg": "Invalid authority"
    },
    {
      "code": 6049,
      "name": "authorityMismatch",
      "msg": "Authority mismatch"
    },
    {
      "code": 6050,
      "name": "invalidSequenceNumber",
      "msg": "Invalid sequence number"
    },
    {
      "code": 6051,
      "name": "invalidPasskeyFormat",
      "msg": "Invalid passkey format"
    },
    {
      "code": 6052,
      "name": "invalidMessageFormat",
      "msg": "Invalid message format"
    },
    {
      "code": 6053,
      "name": "invalidSplitIndex",
      "msg": "Invalid split index"
    },
    {
      "code": 6054,
      "name": "invalidProgramAddress",
      "msg": "Invalid program address"
    },
    {
      "code": 6055,
      "name": "reentrancyDetected",
      "msg": "Reentrancy detected"
    },
    {
      "code": 6056,
      "name": "invalidVaultIndex",
      "msg": "Invalid vault index"
    },
    {
      "code": 6057,
      "name": "insufficientBalance",
      "msg": "Insufficient balance"
    },
    {
      "code": 6058,
      "name": "invalidAction",
      "msg": "Invalid action"
    },
    {
      "code": 6059,
      "name": "insufficientVaultBalance",
      "msg": "Insufficient balance in vault"
    }
  ],
  "types": [
    {
      "name": "config",
      "docs": [
        "LazorKit program configuration and settings",
        "",
        "Stores global program configuration including fee structures, default policy",
        "program, and operational settings. Only the program authority can modify",
        "these settings through the update_config instruction.",
        "",
        "Memory layout optimized for better cache performance:",
        "- Group related fields together",
        "- Align fields to natural boundaries"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "isPaused",
            "docs": [
              "Whether the program is currently paused (1 byte)"
            ],
            "type": "bool"
          },
          {
            "name": "createSmartWalletFee",
            "docs": [
              "Fee charged for creating a new smart wallet (in lamports) (8 bytes)"
            ],
            "type": "u64"
          },
          {
            "name": "feePayerFee",
            "docs": [
              "Fee charged to the fee payer for transactions (in lamports) (8 bytes)"
            ],
            "type": "u64"
          },
          {
            "name": "referralFee",
            "docs": [
              "Fee paid to referral addresses (in lamports) (8 bytes)"
            ],
            "type": "u64"
          },
          {
            "name": "lazorkitFee",
            "docs": [
              "Fee retained by LazorKit protocol (in lamports) (8 bytes)"
            ],
            "type": "u64"
          },
          {
            "name": "authority",
            "docs": [
              "Program authority that can modify configuration settings (32 bytes)"
            ],
            "type": "pubkey"
          },
          {
            "name": "defaultPolicyProgramId",
            "docs": [
              "Default policy program ID for new smart wallets (32 bytes)"
            ],
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "createSmartWalletArgs",
      "docs": [
        "Arguments for creating a new smart wallet",
        "",
        "Contains all necessary parameters for initializing a new smart wallet",
        "with WebAuthn passkey authentication and policy program configuration."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "passkeyPublicKey",
            "docs": [
              "Public key of the WebAuthn passkey for authentication"
            ],
            "type": {
              "array": [
                "u8",
                33
              ]
            }
          },
          {
            "name": "credentialHash",
            "docs": [
              "Unique credential ID from WebAuthn registration"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "initPolicyData",
            "docs": [
              "Policy program initialization data"
            ],
            "type": "bytes"
          },
          {
            "name": "walletId",
            "docs": [
              "Random wallet ID provided by client for uniqueness"
            ],
            "type": "u64"
          },
          {
            "name": "amount",
            "docs": [
              "Initial SOL amount to transfer to the wallet"
            ],
            "type": "u64"
          },
          {
            "name": "referralAddress",
            "docs": [
              "Optional referral address for fee sharing"
            ],
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "vaultIndex",
            "docs": [
              "Random vault index (0-31) calculated off-chain for fee distribution"
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "deviceSlot",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "passkeyPubkey",
            "type": {
              "array": [
                "u8",
                33
              ]
            }
          },
          {
            "name": "credentialHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          }
        ]
      }
    },
    {
      "name": "policyProgramRegistry",
      "docs": [
        "Registry of approved policy programs for smart wallet operations",
        "",
        "Maintains a whitelist of policy programs that can be used to govern",
        "smart wallet transaction validation and security rules."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "registeredPrograms",
            "docs": [
              "List of registered policy program addresses (max 10)"
            ],
            "type": {
              "vec": "pubkey"
            }
          },
          {
            "name": "bump",
            "docs": [
              "Bump seed for PDA derivation and verification"
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "updateType",
      "docs": [
        "Types of configuration parameters that can be updated",
        "",
        "Defines all the configuration parameters that can be modified through",
        "the update_config instruction by the program authority."
      ],
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "createWalletFee"
          },
          {
            "name": "feePayerFee"
          },
          {
            "name": "referralFee"
          },
          {
            "name": "lazorkitFee"
          },
          {
            "name": "defaultPolicyProgram"
          },
          {
            "name": "admin"
          },
          {
            "name": "pauseProgram"
          },
          {
            "name": "unpauseProgram"
          }
        ]
      }
    },
    {
      "name": "walletState",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "walletId",
            "type": "u64"
          },
          {
            "name": "lastNonce",
            "type": "u64"
          },
          {
            "name": "referral",
            "type": "pubkey"
          },
          {
            "name": "policyProgram",
            "type": "pubkey"
          },
          {
            "name": "policyDataLen",
            "type": "u16"
          },
          {
            "name": "policyData",
            "type": "bytes"
          },
          {
            "name": "deviceCount",
            "type": "u8"
          },
          {
            "name": "devices",
            "type": {
              "vec": {
                "defined": {
                  "name": "deviceSlot"
                }
              }
            }
          }
        ]
      }
    }
  ]
};
