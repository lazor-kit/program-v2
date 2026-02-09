use crate::common::TestContext;
use anyhow::Result;

pub mod access_control;
pub mod cryptography;
pub mod dos_and_rent;

pub fn run(ctx: &mut TestContext) -> Result<()> {
    // Orchestrate all audit tests
    access_control::run(ctx)?;
    dos_and_rent::run(ctx)?;
    cryptography::run(ctx)?;
    Ok(())
}
