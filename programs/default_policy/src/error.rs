use anchor_lang::error_code;

#[error_code]
pub enum PolicyError {
    #[msg("Invalid passkey format")]
    InvalidPasskey,
    #[msg("Unauthorized to access smart wallet")]
    Unauthorized,
}
