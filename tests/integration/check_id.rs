use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

fn main() {
    let pk = Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap();
    println!("{:?}", pk.to_bytes());
}
