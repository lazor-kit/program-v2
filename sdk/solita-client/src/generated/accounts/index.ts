export * from './AuthorityAccount'
export * from './SessionAccount'
export * from './WalletAccount'

import { WalletAccount } from './WalletAccount'
import { AuthorityAccount } from './AuthorityAccount'
import { SessionAccount } from './SessionAccount'

export const accountProviders = {
  WalletAccount,
  AuthorityAccount,
  SessionAccount,
}
