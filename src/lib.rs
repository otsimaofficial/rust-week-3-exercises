use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        // Simple constructor - just wrap the value
        // This is basic Rust struct creation
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        // OK so Bitcoin has this weird encoding called CompactSize
        // The idea is to save space by using fewer bytes for small numbers
        // Let me break down the rules:

        // Rule 1: If number is 0 to 252 (0xFC), just use 1 byte
        if self.value <= 0xFC {
            // Easy case - just convert to u8 and put in a vector
            vec![self.value as u8]
        }
        // Rule 2: If number is 253 to 65535, use 0xFD prefix + 2 bytes
        else if self.value <= 0xFFFF {
            // Start with the magic prefix 0xFD
            let mut bytes = vec![0xFD];
            // Convert to u16 and add the little-endian bytes
            // Little-endian means least significant byte first
            bytes.extend_from_slice(&(self.value as u16).to_le_bytes());
            bytes
        }
        // Rule 3: If number is 65536 to 4294967295, use 0xFE prefix + 4 bytes
        else if self.value <= 0xFFFFFFFF {
            let mut bytes = vec![0xFE];
            // Convert to u32 and add little-endian bytes
            bytes.extend_from_slice(&(self.value as u32).to_le_bytes());
            bytes
        }
        // Rule 4: For bigger numbers, use 0xFF prefix + 8 bytes
        else {
            let mut bytes = vec![0xFF];
            // Use the full u64 in little-endian
            bytes.extend_from_slice(&self.value.to_le_bytes());
            bytes
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        // This is the reverse of to_bytes()
        // We need to figure out what format was used and decode it

        // First, basic safety check - do we have any bytes at all?
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        // Look at the first byte to determine the format
        let first_byte = bytes[0];

        match first_byte {
            // Case 1: First byte is 0-252, so the value IS the first byte
            0x00..=0xFC => {
                // Super simple - just convert the byte to u64
                Ok((CompactSize::new(first_byte as u64), 1))
            }
            // Case 2: First byte is 0xFD, so next 2 bytes are the value
            0xFD => {
                // Check if we have enough bytes (need 3 total: prefix + 2 data)
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                // Extract bytes 1 and 2, convert from little-endian
                let value = u16::from_le_bytes([bytes[1], bytes[2]]) as u64;
                Ok((CompactSize::new(value), 3)) // consumed 3 bytes total
            }
            // Case 3: First byte is 0xFE, so next 4 bytes are the value
            0xFE => {
                if bytes.len() < 5 {
                    // need 5 total: prefix + 4 data
                    return Err(BitcoinError::InsufficientBytes);
                }
                // Extract 4 bytes and convert from little-endian
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as u64;
                Ok((CompactSize::new(value), 5))
            }
            // Case 4: First byte is 0xFF, so next 8 bytes are the value
            0xFF => {
                if bytes.len() < 9 {
                    // need 9 total: prefix + 8 data
                    return Err(BitcoinError::InsufficientBytes);
                }
                // Extract all 8 bytes for the full u64
                let value = u64::from_le_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Ok((CompactSize::new(value), 9))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // When we serialize a Txid to JSON, we want it as a hex string
        // Bitcoin txids are always shown as hex strings (like "a1b2c3d4...")
        // The hex crate converts bytes to hex strings
        let hex_string = hex::encode(self.0);
        serializer.serialize_str(&hex_string)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // This is the reverse - convert hex string back to bytes
        // First get the string from JSON
        let hex_string = String::deserialize(deserializer)?;

        // Try to decode the hex string to bytes
        let bytes = hex::decode(&hex_string).map_err(serde::de::Error::custom)?;

        // Bitcoin txids are always exactly 32 bytes
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Txid must be exactly 32 bytes"));
        }

        // Convert Vec<u8> to [u8; 32] array
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes);
        Ok(Txid(txid))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32, // vout = "vector out" = output index
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        // OutPoint identifies a specific output of a transaction
        // It's like saying "the 3rd output of transaction ABC123"
        Self {
            txid: Txid(txid), // wrap the raw bytes in our Txid struct
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        // Bitcoin format: txid (32 bytes) + vout (4 bytes little-endian)
        // Total: 36 bytes
        let mut bytes = Vec::with_capacity(36); // pre-allocate for efficiency

        // First 32 bytes: the transaction ID
        bytes.extend_from_slice(&self.txid.0);

        // Next 4 bytes: the output index in little-endian
        bytes.extend_from_slice(&self.vout.to_le_bytes());

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        // Need exactly 36 bytes for an OutPoint
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }

        // Extract txid from first 32 bytes
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);

        // Extract vout from next 4 bytes (little-endian)
        let vout = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);

        Ok((OutPoint::new(txid, vout), 36)) // consumed 36 bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes } // Basic constructor to create a Script from raw bytes
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let len = CompactSize::new(self.bytes.len() as u64); // Use CompactSize to encode the length of the script
        // First serialize the length using CompactSize
        result.extend(len.to_bytes());
        result.extend(&self.bytes);
        result // Combine CompactSize length prefix with the actual script bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (len_prefix, offset) = CompactSize::from_bytes(bytes)?;
        let len = len_prefix.value as usize; // Get the length of the script from CompactSize

        if bytes.len() < offset + len {
            return Err(BitcoinError::InsufficientBytes);
        } // Ensure i have enough bytes for the script

        let script_bytes = bytes[offset..offset + len].to_vec();
        Ok((Script::new(script_bytes), offset + len)) // Return the script and how many bytes i consumed
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes // Allow using Script as if it were a Vec<u8>
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        } // Basic constructor to create a TransactionInput
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new(); // Start with an empty vector to hold the serialized bytes
        // Serialize the previous output (OutPoint)
        // This is the transaction ID and output index
        // i use the OutPoint's to_bytes() method to get its byte representation
        // Then i serialize the scriptSig (Script) and sequence number
        // The scriptSig is the script that proves ownership of the previous output
        // Finally, i add the sequence number (4 bytes little-endian)
        bytes.extend(self.previous_output.to_bytes());
        bytes.extend(self.script_sig.to_bytes());
        bytes.extend(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (outpoint, offset1) = OutPoint::from_bytes(bytes)?;
        let (script_sig, offset2) = Script::from_bytes(&bytes[offset1..])?;
        let total_offset = offset1 + offset2;

        if bytes.len() < total_offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        } // Ensure i have enough bytes for the sequence

        let sequence = u32::from_le_bytes([
            bytes[total_offset],
            bytes[total_offset + 1],
            bytes[total_offset + 2],
            bytes[total_offset + 3],
        ]);

        Ok((
            TransactionInput::new(outpoint, script_sig, sequence),
            total_offset + 4,
        )) // Return the TransactionInput and how many bytes were consumed
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Version
        bytes.extend(&self.version.to_le_bytes());

        // Input count
        let count = CompactSize::new(self.inputs.len() as u64);
        bytes.extend(count.to_bytes());

        // Inputs
        for input in &self.inputs {
            bytes.extend(input.to_bytes());
        }

        // Lock time
        bytes.extend(&self.lock_time.to_le_bytes());

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let (count_cs, offset1) = CompactSize::from_bytes(&bytes[4..])?;
        let count = count_cs.value as usize;
        let mut inputs = Vec::with_capacity(count);

        let mut offset = 4 + offset1;
        for _ in 0..count {
            let (input, used) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += used;
        }

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let lock_time = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);

        Ok((
            BitcoinTransaction::new(version, inputs, lock_time),
            offset + 4,
        ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        for input in &self.inputs {
            writeln!(f, "Previous Output Vout: {}", input.previous_output.vout)?;
        }
        writeln!(f, "Lock Time: {}", self.lock_time)
    }
}
