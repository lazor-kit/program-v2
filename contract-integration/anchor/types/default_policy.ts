/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/default_policy.json`.
 */
export type DefaultPolicy = {
  address: 'CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE';
  metadata: {
    name: 'defaultPolicy';
    version: '0.1.0';
    spec: '0.1.0';
    description: 'Created with Anchor';
  };
  instructions: [
    {
      name: 'addDevice';
      discriminator: [21, 27, 66, 42, 18, 30, 14, 18];
      accounts: [
        {
          name: 'smartWallet';
          writable: true;
          signer: true;
        },
        {
          name: 'walletDevice';
          signer: true;
        },
        {
          name: 'newWalletDevice';
          writable: true;
        },
        {
          name: 'policy';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [112, 111, 108, 105, 99, 121];
              },
              {
                kind: 'account';
                path: 'walletDevice';
              }
            ];
          };
        },
        {
          name: 'newPolicy';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [112, 111, 108, 105, 99, 121];
              },
              {
                kind: 'account';
                path: 'newWalletDevice';
              }
            ];
          };
        },
        {
          name: 'lazorkit';
          address: 'J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [];
    },
    {
      name: 'checkPolicy';
      discriminator: [28, 88, 170, 179, 239, 136, 25, 35];
      accounts: [
        {
          name: 'walletDevice';
          signer: true;
        },
        {
          name: 'smartWallet';
        },
        {
          name: 'policy';
          writable: true;
        }
      ];
      args: [
        {
          name: 'walletId';
          type: 'u64';
        },
        {
          name: 'passkeyPublicKey';
          type: {
            array: ['u8', 33];
          };
        }
      ];
    },
    {
      name: 'initPolicy';
      discriminator: [45, 234, 110, 100, 209, 146, 191, 86];
      accounts: [
        {
          name: 'smartWallet';
          writable: true;
          signer: true;
        },
        {
          name: 'walletDevice';
          writable: true;
        },
        {
          name: 'policy';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [112, 111, 108, 105, 99, 121];
              },
              {
                kind: 'account';
                path: 'walletDevice';
              }
            ];
          };
        },
        {
          name: 'lazorkit';
          address: 'J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W';
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        }
      ];
      args: [
        {
          name: 'walletId';
          type: 'u64';
        },
        {
          name: 'passkeyPublicKey';
          type: {
            array: ['u8', 33];
          };
        }
      ];
    }
  ];
  accounts: [
    {
      name: 'policy';
      discriminator: [222, 135, 7, 163, 235, 177, 33, 68];
    },
    {
      name: 'walletDevice';
      discriminator: [35, 85, 31, 31, 179, 48, 136, 123];
    }
  ];
  errors: [
    {
      code: 6000;
      name: 'invalidPasskey';
      msg: 'Invalid passkey format';
    },
    {
      code: 6001;
      name: 'unauthorized';
      msg: 'Unauthorized to access smart wallet';
    }
  ];
  types: [
    {
      name: 'policy';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'smartWallet';
            type: 'pubkey';
          },
          {
            name: 'walletDevice';
            type: 'pubkey';
          }
        ];
      };
    },
    {
      name: 'walletDevice';
      docs: [
        'Account that stores a wallet device (passkey) for smart wallet authentication',
        '',
        'Each wallet device represents a WebAuthn passkey that can be used to authenticate',
        'transactions for a specific smart wallet. Multiple devices can be associated with',
        'a single smart wallet for enhanced security and convenience.'
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'passkeyPublicKey';
            docs: [
              'Public key of the WebAuthn passkey for transaction authorization'
            ];
            type: {
              array: ['u8', 33];
            };
          },
          {
            name: 'smartWalletAddress';
            docs: ['Smart wallet address this device is associated with'];
            type: 'pubkey';
          },
          {
            name: 'credentialId';
            docs: ['Unique credential ID from WebAuthn registration'];
            type: 'bytes';
          },
          {
            name: 'bump';
            docs: ['Bump seed for PDA derivation and verification'];
            type: 'u8';
          }
        ];
      };
    }
  ];
};
