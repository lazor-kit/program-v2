/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/lazorkit.json`.
 */
export type Lazorkit = {
  address: 'J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W';
  metadata: {
    name: 'lazorkit';
    version: '0.1.0';
    spec: '0.1.0';
    description: 'Created with Anchor';
  };
  docs: [
    'The Lazor Kit program provides smart wallet functionality with passkey authentication'
  ];
  instructions: [
    {
      name: 'authorizeEphemeralExecution';
      docs: ['Authorize ephemeral execution for temporary program access'];
      discriminator: [220, 152, 90, 147, 146, 90, 72, 115];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'walletDevice';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  119,
                  97,
                  108,
                  108,
                  101,
                  116,
                  95,
                  100,
                  101,
                  118,
                  105,
                  99,
                  101
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              },
              {
                kind: 'arg';
                path: 'args.passkey_public_key.to_hashed_bytes(smart_wallet';
              }
            ];
          };
        },
        {
          name: 'ephemeralAuthorization';
          docs: ['New ephemeral authorization account (rent payer: payer)'];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  101,
                  112,
                  104,
                  101,
                  109,
                  101,
                  114,
                  97,
                  108,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              },
              {
                kind: 'arg';
                path: 'args.ephemeral_public_key';
              }
            ];
          };
        },
        {
          name: 'ixSysvar';
          address: 'Sysvar1nstructions1111111111111111111111111';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'args';
          type: {
            defined: {
              name: 'authorizeEphemeralExecutionArgs';
            };
          };
        }
      ];
    },
    {
      name: 'createDeferredExecution';
      discriminator: [78, 46, 57, 47, 157, 183, 68, 164];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'walletDevice';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  119,
                  97,
                  108,
                  108,
                  101,
                  116,
                  95,
                  100,
                  101,
                  118,
                  105,
                  99,
                  101
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              },
              {
                kind: 'arg';
                path: 'args.passkey_public_key.to_hashed_bytes(smart_wallet';
              }
            ];
          };
        },
        {
          name: 'policyProgramRegistry';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        },
        {
          name: 'policyProgram';
          docs: [
            'Policy program for optional policy enforcement at session creation'
          ];
        },
        {
          name: 'transactionSession';
          docs: ['New transaction session account (rent payer: payer)'];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  116,
                  114,
                  97,
                  110,
                  115,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.last_nonce';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'ixSysvar';
          address: 'Sysvar1nstructions1111111111111111111111111';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'args';
          type: {
            defined: {
              name: 'createDeferredExecutionArgs';
            };
          };
        }
      ];
    },
    {
      name: 'createSmartWallet';
      docs: ['Create a new smart wallet with passkey authentication'];
      discriminator: [129, 39, 235, 18, 132, 68, 203, 19];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'policyProgramRegistry';
          docs: ['Policy program registry'];
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          docs: ['The smart wallet address PDA being created with random ID'];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'args.wallet_id';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          docs: ['Smart wallet data'];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'walletDevice';
          docs: ['Wallet device for the passkey'];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  119,
                  97,
                  108,
                  108,
                  101,
                  116,
                  95,
                  100,
                  101,
                  118,
                  105,
                  99,
                  101
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              },
              {
                kind: 'arg';
                path: 'args.passkey_public_key.to_hashed_bytes(smart_wallet';
              }
            ];
          };
        },
        {
          name: 'config';
          docs: ['Program configuration'];
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'defaultPolicyProgram';
          docs: ['Default policy program for the smart wallet'];
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'args';
          type: {
            defined: {
              name: 'createSmartWalletArgs';
            };
          };
        }
      ];
    },
    {
      name: 'executeDeferredTransaction';
      discriminator: [165, 130, 174, 92, 162, 205, 131, 241];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'referral';
          writable: true;
        },
        {
          name: 'lazorkitVault';
          docs: [
            'LazorKit vault (empty PDA that holds SOL) - random vault selected by client'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'vaultIndex';
              }
            ];
          };
        },
        {
          name: 'transactionSession';
          docs: [
            'Transaction session to execute. Closed on success to refund rent.'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  116,
                  114,
                  97,
                  110,
                  115,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              },
              {
                kind: 'account';
                path: 'transaction_session.authorized_nonce';
                account: 'transactionSession';
              }
            ];
          };
        },
        {
          name: 'sessionRefund';
          writable: true;
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'instructionDataList';
          type: {
            vec: 'bytes';
          };
        },
        {
          name: 'splitIndex';
          type: 'bytes';
        },
        {
          name: 'vaultIndex';
          type: 'u8';
        }
      ];
    },
    {
      name: 'executeDirectTransaction';
      discriminator: [133, 33, 175, 46, 56, 92, 169, 220];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'referral';
          writable: true;
        },
        {
          name: 'lazorkitVault';
          docs: [
            'LazorKit vault (empty PDA that holds SOL) - random vault selected by client'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'args.vault_index';
              }
            ];
          };
        },
        {
          name: 'walletDevice';
        },
        {
          name: 'policyProgramRegistry';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        },
        {
          name: 'policyProgram';
        },
        {
          name: 'cpiProgram';
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'ixSysvar';
          address: 'Sysvar1nstructions1111111111111111111111111';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'args';
          type: {
            defined: {
              name: 'executeDirectTransactionArgs';
            };
          };
        }
      ];
    },
    {
      name: 'executeEphemeralAuthorization';
      docs: ['Execute transactions using ephemeral authorization'];
      discriminator: [34, 195, 199, 141, 192, 147, 156, 14];
      accounts: [
        {
          name: 'feePayer';
          docs: ['Fee payer for the transaction (stored in authorization)'];
          writable: true;
          signer: true;
        },
        {
          name: 'ephemeralSigner';
          docs: ['Ephemeral key that can sign transactions (must be signer)'];
          signer: true;
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'referral';
          writable: true;
        },
        {
          name: 'lazorkitVault';
          docs: [
            'LazorKit vault (empty PDA that holds SOL) - random vault selected by client'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'vaultIndex';
              }
            ];
          };
        },
        {
          name: 'ephemeralAuthorization';
          docs: [
            'Ephemeral authorization to execute. Closed on success to refund rent.'
          ];
          writable: true;
        },
        {
          name: 'authorizationRefund';
          writable: true;
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'instructionDataList';
          type: {
            vec: 'bytes';
          };
        },
        {
          name: 'splitIndex';
          type: 'bytes';
        },
        {
          name: 'vaultIndex';
          type: 'u8';
        }
      ];
    },
    {
      name: 'initializeProgram';
      docs: ['Initialize the program by creating the sequence tracker'];
      discriminator: [176, 107, 205, 168, 24, 157, 175, 103];
      accounts: [
        {
          name: 'signer';
          docs: [
            'The signer of the transaction, who will be the initial authority.'
          ];
          writable: true;
          signer: true;
        },
        {
          name: 'config';
          docs: ["The program's configuration account."];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'policyProgramRegistry';
          docs: [
            'The registry of policy programs that can be used with smart wallets.'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        },
        {
          name: 'defaultPolicyProgram';
          docs: [
            'The default policy program to be used for new smart wallets.'
          ];
        },
        {
          name: 'systemProgram';
          docs: ['The system program.'];
          address: '11111111111111111111111111111111';
        }
      ];
      args: [];
    },
    {
      name: 'invokeWalletPolicy';
      discriminator: [86, 172, 240, 211, 83, 157, 165, 98];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'referral';
          writable: true;
        },
        {
          name: 'lazorkitVault';
          docs: [
            'LazorKit vault (empty PDA that holds SOL) - random vault selected by client'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'args.vault_index';
              }
            ];
          };
        },
        {
          name: 'walletDevice';
        },
        {
          name: 'policyProgram';
        },
        {
          name: 'policyProgramRegistry';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        },
        {
          name: 'ixSysvar';
          address: 'Sysvar1nstructions1111111111111111111111111';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'args';
          type: {
            defined: {
              name: 'invokeWalletPolicyArgs';
            };
          };
        }
      ];
    },
    {
      name: 'manageVault';
      docs: ['Withdraw SOL from vault'];
      discriminator: [165, 7, 106, 242, 73, 193, 195, 128];
      accounts: [
        {
          name: 'authority';
          docs: ['The current authority of the program.'];
          writable: true;
          signer: true;
          relations: ['config'];
        },
        {
          name: 'config';
          docs: ["The program's configuration account."];
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'vault';
          docs: ['Individual vault PDA (empty account that holds SOL)'];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'index';
              }
            ];
          };
        },
        {
          name: 'destination';
          writable: true;
        },
        {
          name: 'systemProgram';
          docs: ['System program'];
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'action';
          type: 'u8';
        },
        {
          name: 'amount';
          type: 'u64';
        },
        {
          name: 'index';
          type: 'u8';
        }
      ];
    },
    {
      name: 'registerPolicyProgram';
      docs: ['Add a program to the policy program registry'];
      discriminator: [15, 54, 85, 112, 89, 180, 121, 13];
      accounts: [
        {
          name: 'authority';
          writable: true;
          signer: true;
          relations: ['config'];
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'policyProgramRegistry';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        }
      ];
      args: [];
    },
    {
      name: 'updateProgramConfig';
      docs: ['Update the program configuration'];
      discriminator: [214, 3, 187, 98, 170, 106, 33, 45];
      accounts: [
        {
          name: 'authority';
          docs: ['The current authority of the program.'];
          writable: true;
          signer: true;
          relations: ['config'];
        },
        {
          name: 'config';
          docs: ["The program's configuration account."];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        }
      ];
      args: [
        {
          name: 'param';
          type: {
            defined: {
              name: 'configUpdateType';
            };
          };
        },
        {
          name: 'value';
          type: 'u64';
        }
      ];
    },
    {
      name: 'updateWalletPolicy';
      discriminator: [90, 225, 16, 40, 95, 80, 20, 107];
      accounts: [
        {
          name: 'payer';
          writable: true;
          signer: true;
        },
        {
          name: 'config';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [99, 111, 110, 102, 105, 103];
              }
            ];
          };
        },
        {
          name: 'smartWallet';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'account';
                path: 'smart_wallet_data.wallet_id';
                account: 'smartWalletData';
              }
            ];
          };
        },
        {
          name: 'smartWalletData';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                  116,
                  95,
                  100,
                  97,
                  116,
                  97
                ];
              },
              {
                kind: 'account';
                path: 'smartWallet';
              }
            ];
          };
        },
        {
          name: 'referral';
          writable: true;
        },
        {
          name: 'lazorkitVault';
          docs: [
            'LazorKit vault (empty PDA that holds SOL) - random vault selected by client'
          ];
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              },
              {
                kind: 'arg';
                path: 'args.vault_index';
              }
            ];
          };
        },
        {
          name: 'walletDevice';
        },
        {
          name: 'oldPolicyProgram';
        },
        {
          name: 'newPolicyProgram';
        },
        {
          name: 'policyProgramRegistry';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
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
                ];
              }
            ];
          };
        },
        {
          name: 'ixSysvar';
          docs: ['CHECK'];
          address: 'Sysvar1nstructions1111111111111111111111111';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'args';
          type: {
            defined: {
              name: 'updateWalletPolicyArgs';
            };
          };
        }
      ];
    }
  ];
  accounts: [
    {
      name: 'ephemeralAuthorization';
      discriminator: [159, 254, 58, 207, 22, 91, 56, 255];
    },
    {
      name: 'policyProgramRegistry';
      discriminator: [158, 67, 114, 157, 27, 153, 86, 72];
    },
    {
      name: 'programConfig';
      discriminator: [196, 210, 90, 231, 144, 149, 140, 63];
    },
    {
      name: 'smartWalletData';
      discriminator: [124, 86, 202, 243, 63, 150, 66, 22];
    },
    {
      name: 'transactionSession';
      discriminator: [169, 116, 227, 43, 10, 34, 251, 2];
    },
    {
      name: 'walletDevice';
      discriminator: [35, 85, 31, 31, 179, 48, 136, 123];
    }
  ];
  errors: [
    {
      code: 6000;
      name: 'passkeyMismatch';
      msg: 'Passkey public key mismatch with stored authenticator';
    },
    {
      code: 6001;
      name: 'smartWalletDataMismatch';
      msg: 'Smart wallet address mismatch with authenticator';
    },
    {
      code: 6002;
      name: 'authenticatorNotFound';
      msg: 'Smart wallet authenticator account not found or invalid';
    },
    {
      code: 6003;
      name: 'secp256r1InvalidLength';
      msg: 'Secp256r1 instruction has invalid data length';
    },
    {
      code: 6004;
      name: 'secp256r1HeaderMismatch';
      msg: 'Secp256r1 instruction header validation failed';
    },
    {
      code: 6005;
      name: 'secp256r1DataMismatch';
      msg: 'Secp256r1 signature data validation failed';
    },
    {
      code: 6006;
      name: 'secp256r1InstructionNotFound';
      msg: 'Secp256r1 instruction not found at specified index';
    },
    {
      code: 6007;
      name: 'invalidSignature';
      msg: 'Invalid signature provided for passkey verification';
    },
    {
      code: 6008;
      name: 'clientDataInvalidUtf8';
      msg: 'Client data JSON is not valid UTF-8';
    },
    {
      code: 6009;
      name: 'clientDataJsonParseError';
      msg: 'Client data JSON parsing failed';
    },
    {
      code: 6010;
      name: 'challengeMissing';
      msg: 'Challenge field missing from client data JSON';
    },
    {
      code: 6011;
      name: 'challengeBase64DecodeError';
      msg: 'Challenge base64 decoding failed';
    },
    {
      code: 6012;
      name: 'challengeDeserializationError';
      msg: 'Challenge message deserialization failed';
    },
    {
      code: 6013;
      name: 'timestampTooOld';
      msg: 'Message timestamp is too far in the past';
    },
    {
      code: 6014;
      name: 'timestampTooNew';
      msg: 'Message timestamp is too far in the future';
    },
    {
      code: 6015;
      name: 'nonceMismatch';
      msg: 'Nonce mismatch: expected different value';
    },
    {
      code: 6016;
      name: 'nonceOverflow';
      msg: 'Nonce overflow: cannot increment further';
    },
    {
      code: 6017;
      name: 'policyProgramNotRegistered';
      msg: 'Policy program not found in registry';
    },
    {
      code: 6018;
      name: 'whitelistFull';
      msg: 'The policy program registry is full.';
    },
    {
      code: 6019;
      name: 'policyDataRequired';
      msg: 'Policy data is required but not provided';
    },
    {
      code: 6020;
      name: 'invalidCheckPolicyDiscriminator';
      msg: 'Invalid instruction discriminator for check_policy';
    },
    {
      code: 6021;
      name: 'invalidDestroyDiscriminator';
      msg: 'Invalid instruction discriminator for destroy';
    },
    {
      code: 6022;
      name: 'invalidInitPolicyDiscriminator';
      msg: 'Invalid instruction discriminator for init_policy';
    },
    {
      code: 6023;
      name: 'policyProgramsIdentical';
      msg: 'Old and new policy programs are identical';
    },
    {
      code: 6024;
      name: 'noDefaultPolicyProgram';
      msg: 'Neither old nor new policy program is the default';
    },
    {
      code: 6025;
      name: 'invalidRemainingAccounts';
      msg: 'Invalid remaining accounts';
    },
    {
      code: 6026;
      name: 'cpiDataMissing';
      msg: 'CPI data is required but not provided';
    },
    {
      code: 6027;
      name: 'invalidCpiData';
      msg: 'CPI data is invalid or malformed';
    },
    {
      code: 6028;
      name: 'insufficientPolicyAccounts';
      msg: 'Insufficient remaining accounts for policy instruction';
    },
    {
      code: 6029;
      name: 'insufficientCpiAccounts';
      msg: 'Insufficient remaining accounts for CPI instruction';
    },
    {
      code: 6030;
      name: 'accountSliceOutOfBounds';
      msg: 'Account slice index out of bounds';
    },
    {
      code: 6031;
      name: 'solTransferInsufficientAccounts';
      msg: 'SOL transfer requires at least 2 remaining accounts';
    },
    {
      code: 6032;
      name: 'newWalletDeviceMissing';
      msg: 'New authenticator account is required but not provided';
    },
    {
      code: 6033;
      name: 'newWalletDevicePasskeyMissing';
      msg: 'New authenticator passkey is required but not provided';
    },
    {
      code: 6034;
      name: 'insufficientLamports';
      msg: 'Insufficient lamports for requested transfer';
    },
    {
      code: 6035;
      name: 'transferAmountOverflow';
      msg: 'Transfer amount would cause arithmetic overflow';
    },
    {
      code: 6036;
      name: 'invalidBumpSeed';
      msg: 'Invalid bump seed for PDA derivation';
    },
    {
      code: 6037;
      name: 'invalidAccountOwner';
      msg: 'Account owner verification failed';
    },
    {
      code: 6038;
      name: 'invalidAccountDiscriminator';
      msg: 'Account discriminator mismatch';
    },
    {
      code: 6039;
      name: 'invalidProgramId';
      msg: 'Invalid program ID';
    },
    {
      code: 6040;
      name: 'programNotExecutable';
      msg: 'Program not executable';
    },
    {
      code: 6041;
      name: 'walletDeviceAlreadyInitialized';
      msg: 'Wallet device already initialized';
    },
    {
      code: 6042;
      name: 'credentialIdTooLarge';
      msg: 'Credential ID exceeds maximum allowed size';
    },
    {
      code: 6043;
      name: 'credentialIdEmpty';
      msg: 'Credential ID cannot be empty';
    },
    {
      code: 6044;
      name: 'policyDataTooLarge';
      msg: 'Policy data exceeds maximum allowed size';
    },
    {
      code: 6045;
      name: 'cpiDataTooLarge';
      msg: 'CPI data exceeds maximum allowed size';
    },
    {
      code: 6046;
      name: 'tooManyRemainingAccounts';
      msg: 'Too many remaining accounts provided';
    },
    {
      code: 6047;
      name: 'invalidPdaDerivation';
      msg: 'Invalid PDA derivation';
    },
    {
      code: 6048;
      name: 'transactionTooOld';
      msg: 'Transaction is too old';
    },
    {
      code: 6049;
      name: 'rateLimitExceeded';
      msg: 'Rate limit exceeded';
    },
    {
      code: 6050;
      name: 'invalidAccountData';
      msg: 'Invalid account data';
    },
    {
      code: 6051;
      name: 'unauthorized';
      msg: 'Unauthorized access attempt';
    },
    {
      code: 6052;
      name: 'programPaused';
      msg: 'Program is paused';
    },
    {
      code: 6053;
      name: 'invalidInstructionData';
      msg: 'Invalid instruction data';
    },
    {
      code: 6054;
      name: 'accountAlreadyInitialized';
      msg: 'Account already initialized';
    },
    {
      code: 6055;
      name: 'accountNotInitialized';
      msg: 'Account not initialized';
    },
    {
      code: 6056;
      name: 'invalidAccountState';
      msg: 'Invalid account state';
    },
    {
      code: 6057;
      name: 'integerOverflow';
      msg: 'Operation would cause integer overflow';
    },
    {
      code: 6058;
      name: 'integerUnderflow';
      msg: 'Operation would cause integer underflow';
    },
    {
      code: 6059;
      name: 'invalidFeeAmount';
      msg: 'Invalid fee amount';
    },
    {
      code: 6060;
      name: 'insufficientBalanceForFee';
      msg: 'Insufficient balance for fee';
    },
    {
      code: 6061;
      name: 'invalidAuthority';
      msg: 'Invalid authority';
    },
    {
      code: 6062;
      name: 'authorityMismatch';
      msg: 'Authority mismatch';
    },
    {
      code: 6063;
      name: 'invalidSequenceNumber';
      msg: 'Invalid sequence number';
    },
    {
      code: 6064;
      name: 'duplicateTransaction';
      msg: 'Duplicate transaction detected';
    },
    {
      code: 6065;
      name: 'invalidTransactionOrdering';
      msg: 'Invalid transaction ordering';
    },
    {
      code: 6066;
      name: 'maxWalletLimitReached';
      msg: 'Maximum wallet limit reached';
    },
    {
      code: 6067;
      name: 'invalidWalletConfiguration';
      msg: 'Invalid wallet configuration';
    },
    {
      code: 6068;
      name: 'walletNotFound';
      msg: 'Wallet not found';
    },
    {
      code: 6069;
      name: 'invalidPasskeyFormat';
      msg: 'Invalid passkey format';
    },
    {
      code: 6070;
      name: 'passkeyAlreadyRegistered';
      msg: 'Passkey already registered';
    },
    {
      code: 6071;
      name: 'invalidMessageFormat';
      msg: 'Invalid message format';
    },
    {
      code: 6072;
      name: 'messageSizeExceedsLimit';
      msg: 'Message size exceeds limit';
    },
    {
      code: 6073;
      name: 'invalidSplitIndex';
      msg: 'Invalid split index';
    },
    {
      code: 6074;
      name: 'cpiExecutionFailed';
      msg: 'CPI execution failed';
    },
    {
      code: 6075;
      name: 'invalidProgramAddress';
      msg: 'Invalid program address';
    },
    {
      code: 6076;
      name: 'whitelistOperationFailed';
      msg: 'Whitelist operation failed';
    },
    {
      code: 6077;
      name: 'invalidWhitelistState';
      msg: 'Invalid whitelist state';
    },
    {
      code: 6078;
      name: 'emergencyShutdown';
      msg: 'Emergency shutdown activated';
    },
    {
      code: 6079;
      name: 'recoveryModeRequired';
      msg: 'Recovery mode required';
    },
    {
      code: 6080;
      name: 'invalidRecoveryAttempt';
      msg: 'Invalid recovery attempt';
    },
    {
      code: 6081;
      name: 'auditLogFull';
      msg: 'Audit log full';
    },
    {
      code: 6082;
      name: 'invalidAuditEntry';
      msg: 'Invalid audit entry';
    },
    {
      code: 6083;
      name: 'reentrancyDetected';
      msg: 'Reentrancy detected';
    },
    {
      code: 6084;
      name: 'invalidCallDepth';
      msg: 'Invalid call depth';
    },
    {
      code: 6085;
      name: 'stackOverflowProtection';
      msg: 'Stack overflow protection triggered';
    },
    {
      code: 6086;
      name: 'memoryLimitExceeded';
      msg: 'Memory limit exceeded';
    },
    {
      code: 6087;
      name: 'computationLimitExceeded';
      msg: 'Computation limit exceeded';
    },
    {
      code: 6088;
      name: 'invalidRentExemption';
      msg: 'Invalid rent exemption';
    },
    {
      code: 6089;
      name: 'accountClosureFailed';
      msg: 'Account closure failed';
    },
    {
      code: 6090;
      name: 'invalidAccountClosure';
      msg: 'Invalid account closure';
    },
    {
      code: 6091;
      name: 'refundFailed';
      msg: 'Refund failed';
    },
    {
      code: 6092;
      name: 'invalidRefundAmount';
      msg: 'Invalid refund amount';
    },
    {
      code: 6093;
      name: 'allVaultsFull';
      msg: 'All vault slots are full';
    },
    {
      code: 6094;
      name: 'vaultNotFound';
      msg: 'Vault not found for the specified mint';
    },
    {
      code: 6095;
      name: 'insufficientVaultBalance';
      msg: 'Insufficient balance in vault';
    },
    {
      code: 6096;
      name: 'vaultOverflow';
      msg: 'Vault balance overflow';
    },
    {
      code: 6097;
      name: 'invalidVaultIndex';
      msg: 'Invalid vault index';
    },
    {
      code: 6098;
      name: 'insufficientBalance';
      msg: 'Insufficient balance';
    },
    {
      code: 6099;
      name: 'invalidAction';
      msg: 'Invalid action';
    }
  ];
  types: [
    {
      name: 'authorizeEphemeralExecutionArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'signature';
            type: 'bytes';
          },
          {
            name: 'clientDataJsonRaw';
            type: 'bytes';
          },
          {
            name: 'authenticatorDataRaw';
            type: 'bytes';
          },
          {
            name: 'verifyInstructionIndex';
            type: 'u8';
          },
          {
            name: 'ephemeralPublicKey';
            type: 'pubkey';
          },
          {
            name: 'expiresAt';
            type: 'i64';
          },
          {
            name: 'vaultIndex';
            type: 'u8';
          },
          {
            name: 'instructionDataList';
            type: {
              vec: 'bytes';
            };
          },
          {
            name: 'splitIndex';
            type: 'bytes';
          }
        ];
      };
    },
    {
      name: 'configUpdateType';
      type: {
        kind: 'enum';
        variants: [
          {
            name: 'createWalletFee';
          },
          {
            name: 'feePayerFee';
          },
          {
            name: 'referralFee';
          },
          {
            name: 'lazorkitFee';
          },
          {
            name: 'defaultPolicyProgram';
          },
          {
            name: 'admin';
          },
          {
            name: 'pauseProgram';
          },
          {
            name: 'unpauseProgram';
          }
        ];
      };
    },
    {
      name: 'createDeferredExecutionArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'signature';
            type: 'bytes';
          },
          {
            name: 'clientDataJsonRaw';
            type: 'bytes';
          },
          {
            name: 'authenticatorDataRaw';
            type: 'bytes';
          },
          {
            name: 'verifyInstructionIndex';
            type: 'u8';
          },
          {
            name: 'policyData';
            type: 'bytes';
          },
          {
            name: 'expiresAt';
            type: 'i64';
          },
          {
            name: 'vaultIndex';
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'createSmartWalletArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'credentialId';
            type: 'bytes';
          },
          {
            name: 'policyData';
            type: 'bytes';
          },
          {
            name: 'walletId';
            type: 'u64';
          },
          {
            name: 'amount';
            type: 'u64';
          },
          {
            name: 'referralAddress';
            type: {
              option: 'pubkey';
            };
          },
          {
            name: 'vaultIndex';
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'ephemeralAuthorization';
      docs: [
        'Ephemeral authorization for temporary program access.',
        'Created after passkey authentication. Allows execution with ephemeral key',
        'for a limited time to authorized programs with multiple instructions.'
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'ownerWalletAddress';
            docs: ['Smart wallet that authorized this session'];
            type: 'pubkey';
          },
          {
            name: 'ephemeralPublicKey';
            docs: ['Ephemeral public key that can sign transactions'];
            type: 'pubkey';
          },
          {
            name: 'expiresAt';
            docs: ['Unix timestamp when this session expires'];
            type: 'i64';
          },
          {
            name: 'feePayerAddress';
            docs: ['Fee payer for transactions in this session'];
            type: 'pubkey';
          },
          {
            name: 'rentRefundAddress';
            docs: ['Where to refund rent when closing the session'];
            type: 'pubkey';
          },
          {
            name: 'vaultIndex';
            docs: ['Vault index for fee collection'];
            type: 'u8';
          },
          {
            name: 'instructionDataHash';
            docs: [
              'Combined hash of all instruction data that can be executed'
            ];
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'accountsMetadataHash';
            docs: ['Combined hash of all accounts that will be used'];
            type: {
              array: ['u8', 32];
            };
          }
        ];
      };
    },
    {
      name: 'executeDirectTransactionArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'signature';
            type: 'bytes';
          },
          {
            name: 'clientDataJsonRaw';
            type: 'bytes';
          },
          {
            name: 'authenticatorDataRaw';
            type: 'bytes';
          },
          {
            name: 'verifyInstructionIndex';
            type: 'u8';
          },
          {
            name: 'splitIndex';
            type: 'u16';
          },
          {
            name: 'policyData';
            type: 'bytes';
          },
          {
            name: 'cpiData';
            type: 'bytes';
          },
          {
            name: 'vaultIndex';
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'invokeWalletPolicyArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'signature';
            type: 'bytes';
          },
          {
            name: 'clientDataJsonRaw';
            type: 'bytes';
          },
          {
            name: 'authenticatorDataRaw';
            type: 'bytes';
          },
          {
            name: 'verifyInstructionIndex';
            type: 'u8';
          },
          {
            name: 'policyData';
            type: 'bytes';
          },
          {
            name: 'newWalletDevice';
            type: {
              option: {
                defined: {
                  name: 'newWalletDeviceArgs';
                };
              };
            };
          },
          {
            name: 'vaultIndex';
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'newWalletDeviceArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'credentialId';
            type: 'bytes';
          }
        ];
      };
    },
    {
      name: 'policyProgramRegistry';
      docs: [
        'Registry of approved policy programs that can govern smart wallet operations'
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'registeredPrograms';
            docs: ['List of registered policy program addresses'];
            type: {
              vec: 'pubkey';
            };
          },
          {
            name: 'bump';
            docs: ['Bump seed for PDA derivation'];
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'programConfig';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'authority';
            type: 'pubkey';
          },
          {
            name: 'createSmartWalletFee';
            type: 'u64';
          },
          {
            name: 'feePayerFee';
            type: 'u64';
          },
          {
            name: 'referralFee';
            type: 'u64';
          },
          {
            name: 'lazorkitFee';
            type: 'u64';
          },
          {
            name: 'defaultPolicyProgramId';
            type: 'pubkey';
          },
          {
            name: 'isPaused';
            type: 'bool';
          }
        ];
      };
    },
    {
      name: 'smartWalletData';
      docs: ['Data account for a smart wallet'];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'walletId';
            docs: ['Unique identifier for this smart wallet'];
            type: 'u64';
          },
          {
            name: 'referralAddress';
            docs: ["Referral address that governs this wallet's operations"];
            type: 'pubkey';
          },
          {
            name: 'policyProgramId';
            docs: ["Policy program that governs this wallet's operations"];
            type: 'pubkey';
          },
          {
            name: 'lastNonce';
            docs: ['Last nonce used for message verification'];
            type: 'u64';
          },
          {
            name: 'bump';
            docs: ['Bump seed for PDA derivation'];
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'transactionSession';
      docs: [
        'Transaction session for deferred execution.',
        'Created after full passkey + policy verification. Contains all bindings',
        'necessary to execute the transaction later without re-verification.'
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'ownerWalletAddress';
            docs: ['Smart wallet that authorized this session'];
            type: 'pubkey';
          },
          {
            name: 'instructionDataHash';
            docs: ['Combined sha256 hash of all transaction instruction data'];
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'accountsMetadataHash';
            docs: [
              'Combined sha256 hash over all ordered remaining account metas plus target programs'
            ];
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'authorizedNonce';
            docs: [
              'The nonce that was authorized at session creation (bound into data hash)'
            ];
            type: 'u64';
          },
          {
            name: 'expiresAt';
            docs: ['Unix expiration timestamp'];
            type: 'i64';
          },
          {
            name: 'rentRefundAddress';
            docs: ['Where to refund rent when closing the session'];
            type: 'pubkey';
          },
          {
            name: 'vaultIndex';
            docs: ['Vault index for fee collection'];
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'updateWalletPolicyArgs';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'signature';
            type: 'bytes';
          },
          {
            name: 'clientDataJsonRaw';
            type: 'bytes';
          },
          {
            name: 'authenticatorDataRaw';
            type: 'bytes';
          },
          {
            name: 'verifyInstructionIndex';
            type: 'u8';
          },
          {
            name: 'splitIndex';
            type: 'u16';
          },
          {
            name: 'destroyPolicyData';
            type: 'bytes';
          },
          {
            name: 'initPolicyData';
            type: 'bytes';
          },
          {
            name: 'newWalletDevice';
            type: {
              option: {
                defined: {
                  name: 'newWalletDeviceArgs';
                };
              };
            };
          },
          {
            name: 'vaultIndex';
            type: 'u8';
          }
        ];
      };
    },
    {
      name: 'walletDevice';
      docs: [
        'Account that stores a wallet device (passkey) used to authenticate to a smart wallet'
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            docs: [
              'The public key of the passkey for this wallet device that can authorize transactions'
            ];
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'smartWalletAddress';
            docs: ['The smart wallet this wallet device belongs to'];
            type: 'pubkey';
          },
          {
            name: 'credentialId';
            docs: ['The credential ID this wallet device belongs to'];
            type: 'bytes';
          },
          {
            name: 'bump';
            docs: ['Bump seed for PDA derivation'];
            type: 'u8';
          }
        ];
      };
    }
  ];
};
