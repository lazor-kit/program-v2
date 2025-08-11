/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/default_rule.json`.
 */
export type DefaultRule = {
  "address": "CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE",
  "metadata": {
    "name": "defaultRule",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "Created with Anchor"
  },
  "instructions": [
    {
      "name": "addDevice",
      "discriminator": [
        21,
        27,
        66,
        42,
        18,
        30,
        14,
        18
      ],
      "accounts": [
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "smartWalletAuthenticator",
          "signer": true
        },
        {
          "name": "newSmartWalletAuthenticator"
        },
        {
          "name": "rule",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  117,
                  108,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "smartWalletAuthenticator"
              }
            ]
          }
        },
        {
          "name": "newRule",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  117,
                  108,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "newSmartWalletAuthenticator"
              }
            ]
          }
        },
        {
          "name": "lazorkit",
          "address": "J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "checkRule",
      "discriminator": [
        215,
        90,
        220,
        175,
        191,
        212,
        144,
        147
      ],
      "accounts": [
        {
          "name": "smartWalletAuthenticator",
          "signer": true
        },
        {
          "name": "rule",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "initRule",
      "discriminator": [
        129,
        224,
        96,
        169,
        247,
        125,
        74,
        118
      ],
      "accounts": [
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "smartWallet"
        },
        {
          "name": "smartWalletAuthenticator",
          "docs": [
            "CHECK"
          ],
          "signer": true
        },
        {
          "name": "rule",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  117,
                  108,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "smartWalletAuthenticator"
              }
            ]
          }
        },
        {
          "name": "lazorkit",
          "address": "J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    }
  ],
  "accounts": [
    {
      "name": "rule",
      "discriminator": [
        82,
        10,
        53,
        40,
        250,
        61,
        143,
        130
      ]
    },
    {
      "name": "smartWalletAuthenticator",
      "discriminator": [
        126,
        36,
        85,
        166,
        77,
        139,
        221,
        129
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "invalidPasskey"
    },
    {
      "code": 6001,
      "name": "unAuthorize"
    }
  ],
  "types": [
    {
      "name": "rule",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "smartWallet",
            "type": "pubkey"
          },
          {
            "name": "smartWalletAuthenticator",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "smartWalletAuthenticator",
      "docs": [
        "Account that stores authentication data for a smart wallet"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "passkeyPubkey",
            "docs": [
              "The public key of the passkey that can authorize transactions"
            ],
            "type": {
              "array": [
                "u8",
                33
              ]
            }
          },
          {
            "name": "smartWallet",
            "docs": [
              "The smart wallet this authenticator belongs to"
            ],
            "type": "pubkey"
          },
          {
            "name": "credentialId",
            "docs": [
              "The credential ID this authenticator belongs to"
            ],
            "type": "bytes"
          },
          {
            "name": "bump",
            "docs": [
              "Bump seed for PDA derivation"
            ],
            "type": "u8"
          }
        ]
      }
    }
  ]
};
