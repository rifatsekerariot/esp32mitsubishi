//! CN105 Checksum implementation.
//!
//! The checksum algorithm used by Mitsubishi CN105 protocol:
//! `(0xFC - sum_of_all_bytes) & 0xFF`
//! where `sum_of_all_bytes` is the wrapping sum of all bytes in the frame excluding the checksum byte.

/// Calculates the CN105 packet checksum.
///
/// # Arguments
/// * `bytes` - Slice containing all bytes of the packet (header + payload), excluding the checksum byte itself.
#[inline]
pub fn calculate_checksum(bytes: &[u8]) -> u8 {
    let sum = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    0xFCu8.wrapping_sub(sum)
}

/// Verifies if a packet's checksum matches its last byte.
///
/// # Arguments
/// * `packet` - Full packet slice including the trailing checksum byte.
#[inline]
pub fn verify_checksum(packet: &[u8]) -> bool {
    if packet.len() < 2 {
        return false;
    }
    let (data, expected) = packet.split_at(packet.len() - 1);
    calculate_checksum(data) == expected[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect_packet_checksum() {
        // Standard connect packet: 0xfc, 0x5a, 0x01, 0x30, 0x02, 0xca, 0x01, 0xa8
        let connect = [0xfc, 0x5a, 0x01, 0x30, 0x02, 0xca, 0x01, 0xa8];
        assert!(verify_checksum(&connect));
        assert_eq!(calculate_checksum(&connect[..7]), 0xa8);
    }

    #[test]
    fn test_arbitrary_checksum() {
        let dummy = [0xfc, 0x41, 0x01, 0x30, 0x10];
        let c = calculate_checksum(&dummy);
        let mut full = dummy.to_vec();
        full.push(c);
        assert!(verify_checksum(&full));
    }
}
