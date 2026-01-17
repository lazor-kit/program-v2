use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
struct TestArray {
    data: [u8; 33],
}

pub fn test() {
    let t = TestArray { data: [0; 33] };
    let mut buf = vec![];
    t.serialize(&mut buf).unwrap();
    let _ = TestArray::try_from(&buf).unwrap();
}
