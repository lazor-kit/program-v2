/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/default_policy.json`.
 */
export type DefaultPolicy = {
  "address": "BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7",
  "metadata": {
    "name": "defaultPolicy",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "Created with Anchor"
  },
  "instructions": [
    {
      "name": "addAuthority",
      "discriminator": [
        229,
        9,
        106,
        73,
        91,
        213,
        109,
        183
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "smartWallet"
        }
      ],
      "args": [
        {
          "name": "policyData",
          "type": "bytes"
        },
        {
          "name": "newAuthority",
          "type": "pubkey"
        }
      ],
      "returns": {
        "defined": {
          "name": "policyStruct"
        }
      }
    },
    {
      "name": "checkPolicy",
      "discriminator": [
        28,
        88,
        170,
        179,
        239,
        136,
        25,
        35
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "smartWallet"
        }
      ],
      "args": [
        {
          "name": "policyData",
          "type": "bytes"
        }
      ]
    },
    {
      "name": "initPolicy",
      "discriminator": [
        45,
        234,
        110,
        100,
        209,
        146,
        191,
        86
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "smartWallet",
          "docs": [
            "Must mut follow lazorkit standard"
          ],
          "writable": true
        }
      ],
      "args": [],
      "returns": {
        "defined": {
          "name": "policyStruct"
        }
      }
    },
    {
      "name": "removeAuthority",
      "discriminator": [
        242,
        104,
        208,
        132,
        190,
        250,
        74,
        216
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "smartWallet"
        }
      ],
      "args": [
        {
          "name": "policyData",
          "type": "bytes"
        },
        {
          "name": "newAuthority",
          "type": "pubkey"
        }
      ],
      "returns": {
        "defined": {
          "name": "policyStruct"
        }
      }
    }
  ],
  "accounts": [
    {
      "name": "walletAuthority",
      "discriminator": [
        77,
        154,
        162,
        218,
        217,
        205,
        216,
        227
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "invalidPasskey",
      "msg": "Invalid passkey format"
    },
    {
      "code": 6001,
      "name": "unauthorized",
      "msg": "Unauthorized to access smart wallet"
    },
    {
      "code": 6002,
      "name": "invalidSmartWallet",
      "msg": "Invalid smart wallet"
    }
  ],
  "types": [
    {
      "name": "policyStruct",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "smartWallet",
            "type": "pubkey"
          },
          {
            "name": "authoritis",
            "type": {
              "vec": "pubkey"
            }
          }
        ]
      }
    },
    {
      "name": "walletAuthority",
      "docs": [
        "Wallet authority account linking a passkey to a smart wallet"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "passkeyPubkey",
            "docs": [
              "Secp256r1 compressed public key (33 bytes)"
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
              "SHA256 hash of the credential ID"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "smartWallet",
            "docs": [
              "Associated smart wallet address"
            ],
            "type": "pubkey"
          },
          {
            "name": "bump",
            "docs": [
              "PDA bump seed"
            ],
            "type": "u8"
          }
        ]
      }
    }
  ],
  "constants": [
    {
      "name": "policyDataSize",
      "type": "u16",
      "value": "196"
    }
  ]
};
