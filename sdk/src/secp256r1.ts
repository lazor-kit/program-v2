import { Address, address } from "@solana/addresses";
import { Instruction } from "@solana/instructions";

const SECP256R1_NATIVE_PROGRAM = address("Secp256r1SigVerify1111111111111111111111111");

const SIGNATURE_OFFSETS_SERIALIZED_SIZE = 14;
const SIGNATURE_OFFSETS_START = 2;
const DATA_START = SIGNATURE_OFFSETS_SERIALIZED_SIZE + SIGNATURE_OFFSETS_START;
const SIGNATURE_SERIALIZED_SIZE = 64;
const COMPRESSED_PUBKEY_SERIALIZED_SIZE = 33;

type Secp256r1SignatureOffsets = {
    signature_offset: number;
    signature_instruction_index: number;
    public_key_offset: number;
    public_key_instruction_index: number;
    message_data_offset: number;
    message_data_size: number;
    message_instruction_index: number;
};

function writeOffsets(view: DataView, offset: number, offsets: Secp256r1SignatureOffsets) {
    view.setUint16(offset + 0, offsets.signature_offset, true);
    view.setUint16(offset + 2, offsets.signature_instruction_index, true);
    view.setUint16(offset + 4, offsets.public_key_offset, true);
    view.setUint16(offset + 6, offsets.public_key_instruction_index, true);
    view.setUint16(offset + 8, offsets.message_data_offset, true);
    view.setUint16(offset + 10, offsets.message_data_size, true);
    view.setUint16(offset + 12, offsets.message_instruction_index, true);
}

export function createSecp256r1VerifyInstruction(
    message: Uint8Array,
    pubkey: Uint8Array, // 33 bytes
    signature: Uint8Array // 64 bytes
): Instruction {
    // Verify lengths
    if (pubkey.length !== COMPRESSED_PUBKEY_SERIALIZED_SIZE) {
        throw new Error(`Invalid key length: ${pubkey.length}. Expected ${COMPRESSED_PUBKEY_SERIALIZED_SIZE}`);
    }
    if (signature.length !== SIGNATURE_SERIALIZED_SIZE) {
        throw new Error(`Invalid signature length: ${signature.length}. Expected ${SIGNATURE_SERIALIZED_SIZE}`);
    }

    // Calculate total size
    const totalSize =
        DATA_START +
        SIGNATURE_SERIALIZED_SIZE +
        COMPRESSED_PUBKEY_SERIALIZED_SIZE +
        message.length;

    const instructionData = new Uint8Array(totalSize);
    const view = new DataView(instructionData.buffer);

    // Calculate offsets
    const numSignatures = 1;
    const publicKeyOffset = DATA_START;
    const signatureOffset = publicKeyOffset + COMPRESSED_PUBKEY_SERIALIZED_SIZE;
    const messageDataOffset = signatureOffset + SIGNATURE_SERIALIZED_SIZE;

    // Write number of signatures (u8) and padding (u8)
    // bytes_of(&[num_signatures, 0]) -> [1, 0]
    instructionData[0] = numSignatures;
    instructionData[1] = 0;

    // Create offsets structure
    const offsets: Secp256r1SignatureOffsets = {
        signature_offset: signatureOffset,
        signature_instruction_index: 0xffff, // u16::MAX
        public_key_offset: publicKeyOffset,
        public_key_instruction_index: 0xffff, // u16::MAX
        message_data_offset: messageDataOffset,
        message_data_size: message.length,
        message_instruction_index: 0xffff, // u16::MAX
    };

    // Write offsets
    writeOffsets(view, SIGNATURE_OFFSETS_START, offsets);

    // Write public key
    instructionData.set(pubkey, publicKeyOffset);

    // Write signature
    instructionData.set(signature, signatureOffset);

    // Write message
    instructionData.set(message, messageDataOffset);

    return {
        programAddress: SECP256R1_NATIVE_PROGRAM,
        accounts: [],
        data: instructionData,
    };
}
