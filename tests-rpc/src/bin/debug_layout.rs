use solana_sdk::pubkey::Pubkey;

fn main() {
    let s = "Secp256r1SigVerify1111111111111111111111111";
    let p = s.parse::<Pubkey>().unwrap();
    println!("BYTES: {:?}", p.to_bytes());
}
