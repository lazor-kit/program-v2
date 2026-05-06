#![allow(unexpected_cfgs)]

#[cfg(not(feature = "no-entrypoint"))]
use {default_env::default_env, solana_security_txt::security_txt};

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "LazorKit Smart Wallet",
    project_url: "https://lazorkit.com",
    contacts: "email:security@lazorkit.app,link:https://github.com/lazor-kit/program-v2/security/advisories/new",
    policy: "https://github.com/lazor-kit/program-v2/blob/main/SECURITY.md",

    preferred_languages: "en,vi",
    source_code: "https://github.com/lazor-kit/program-v2",
    source_revision: default_env!("GITHUB_SHA", ""),
    source_release: default_env!("GITHUB_REF_NAME", ""),
    auditors: "Accretion Labs — https://github.com/lazor-kit/program-v2/blob/main/audits/2026-accretion-solana-foundation-lazorkit-audit-A26SFR1.pdf"
}

pub mod auth;
pub mod compact;
pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod utils;
