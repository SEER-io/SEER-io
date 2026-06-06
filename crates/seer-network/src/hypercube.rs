//! Implements the log(N) hypercube routing matrix for fast-path and slow-path
//! inter-node messaging over the abstract address layout.
//!
//! # Design
//! An N-dimensional hypercube connects 2^N nodes such that any two nodes differ
//! in exactly one bit per hop. This gives an optimal diameter of N hops and
//! exactly N neighbours per node — ideal for low-latency, fault-tolerant routing
//! across a large peer-to-peer network.
//!
//! Each node's position in the hypercube is derived from the low-order bits of
//! its 256-bit ADNL identity, making address assignment deterministic and
//! collision-resistant.

/// The maximum supported hypercube dimensionality (matches genesis default of 10,
/// giving up to 1 024 logical positions).
pub const MAX_DIMENSIONS: usize = 32;

/// A node's logical position within the hypercube, encoded as a bitmask.
/// Only the low-order `dimensions` bits are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HypercubeAddress(pub u32);

impl HypercubeAddress {
    /// Derives a hypercube address from a raw ADNL identity by taking the
    /// low-order 32 bits and masking to the configured dimensionality.
    pub fn from_adnl(adnl_id: &[u8; 32], dimensions: usize) -> Self {
        assert!(dimensions <= MAX_DIMENSIONS, "dimensions exceed maximum");
        let raw = u32::from_le_bytes([adnl_id[0], adnl_id[1], adnl_id[2], adnl_id[3]]);
        let mask = if dimensions == 32 { u32::MAX } else { (1u32 << dimensions) - 1 };
        HypercubeAddress(raw & mask)
    }

    /// Returns the Hamming distance (number of differing bits) between two addresses.
    pub fn hamming_distance(&self, other: &HypercubeAddress) -> u32 {
        (self.0 ^ other.0).count_ones()
    }

    /// Returns the set of direct neighbours (nodes reachable in exactly one hop)
    /// for this address in a hypercube of the given dimensionality.
    pub fn neighbours(&self, dimensions: usize) -> Vec<HypercubeAddress> {
        (0..dimensions)
            .map(|bit| HypercubeAddress(self.0 ^ (1u32 << bit)))
            .collect()
    }
}

/// A single entry in the routing table: a neighbour address paired with its
/// ADNL identity for transport-layer lookup.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub address: HypercubeAddress,
    pub adnl_id: [u8; 32],
}

/// The hypercube routing table for a single local node.
///
/// Maintains one entry per dimension (i.e., one per direct neighbour bit-flip),
/// enabling O(log N) greedy routing to any destination.
#[derive(Debug)]
pub struct HypercubeRouter {
    /// This node's own hypercube address.
    pub local: HypercubeAddress,
    /// Number of dimensions in the hypercube.
    pub dimensions: usize,
    /// Routing table: indexed by dimension (bit position), holds the best known
    /// peer for that bit-flip direction.
    table: Vec<Option<RouteEntry>>,
}

impl HypercubeRouter {
    /// Creates a new router for the given local address and dimensionality.
    pub fn new(local: HypercubeAddress, dimensions: usize) -> Self {
        assert!(dimensions <= MAX_DIMENSIONS);
        Self {
            local,
            dimensions,
            table: vec![None; dimensions],
        }
    }

    /// Registers a peer in the routing table at the appropriate dimension slot.
    /// Only accepts peers that are exactly one bit-flip away from the local node
    /// (i.e., direct hypercube neighbours).
    ///
    /// Returns `true` if the entry was inserted, `false` if the peer is not a
    /// direct neighbour or the slot was already occupied.
    pub fn register_neighbour(&mut self, entry: RouteEntry) -> bool {
        let xor = self.local.0 ^ entry.address.0;
        // Must be a power of two (exactly one bit different).
        if xor == 0 || (xor & (xor - 1)) != 0 {
            return false;
        }
        let bit = xor.trailing_zeros() as usize;
        if bit >= self.dimensions {
            return false;
        }
        if self.table[bit].is_none() {
            self.table[bit] = Some(entry);
            true
        } else {
            false
        }
    }

    /// Performs greedy hypercube routing: given a destination address, returns
    /// the next-hop `RouteEntry` that brings the message one step closer.
    ///
    /// The algorithm selects the dimension bit where `local XOR dest` is set and
    /// a routing entry exists, preferring the highest-order differing bit first
    /// (fast-path). Falls back to any available differing dimension (slow-path).
    ///
    /// Returns `None` if the destination is the local node or no route exists.
    pub fn next_hop(&self, dest: HypercubeAddress) -> Option<&RouteEntry> {
        let xor = self.local.0 ^ dest.0;
        if xor == 0 {
            return None; // Already at destination.
        }
        // Fast-path: highest-order differing bit.
        for bit in (0..self.dimensions).rev() {
            if (xor >> bit) & 1 == 1 {
                if let Some(ref entry) = self.table[bit] {
                    return Some(entry);
                }
            }
        }
        // Slow-path: any differing bit with a known route.
        for bit in 0..self.dimensions {
            if (xor >> bit) & 1 == 1 {
                if let Some(ref entry) = self.table[bit] {
                    return Some(entry);
                }
            }
        }
        None
    }

    /// Returns the number of populated routing table slots.
    pub fn known_neighbours(&self) -> usize {
        self.table.iter().filter(|e| e.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adnl_from_u32(v: u32) -> [u8; 32] {
        let mut id = [0u8; 32];
        let bytes = v.to_le_bytes();
        id[..4].copy_from_slice(&bytes);
        id
    }

    #[test]
    fn test_address_from_adnl() {
        let adnl = adnl_from_u32(0b1111_1111);
        let addr = HypercubeAddress::from_adnl(&adnl, 4);
        assert_eq!(addr.0, 0b1111);
    }

    #[test]
    fn test_hamming_distance() {
        let a = HypercubeAddress(0b0000);
        let b = HypercubeAddress(0b1010);
        assert_eq!(a.hamming_distance(&b), 2);
    }

    #[test]
    fn test_neighbours_count() {
        let addr = HypercubeAddress(0b0000);
        let neighbours = addr.neighbours(4);
        assert_eq!(neighbours.len(), 4);
        // Each neighbour differs by exactly one bit.
        for n in &neighbours {
            assert_eq!(addr.hamming_distance(n), 1);
        }
    }

    #[test]
    fn test_register_and_route() {
        // 3-dimensional hypercube: local = 000 (0), dest = 111 (7).
        let local = HypercubeAddress(0b000);
        let mut router = HypercubeRouter::new(local, 3);

        // Register all three direct neighbours.
        for bit in 0..3u32 {
            let addr = HypercubeAddress(1u32 << bit);
            let entry = RouteEntry { address: addr, adnl_id: adnl_from_u32(1 << bit) };
            assert!(router.register_neighbour(entry));
        }
        assert_eq!(router.known_neighbours(), 3);

        // Route to 111: first hop should flip the highest set bit (bit 2 → address 100).
        let dest = HypercubeAddress(0b111);
        let hop = router.next_hop(dest).expect("route should exist");
        assert_eq!(hop.address.0, 0b100);
    }

    #[test]
    fn test_no_route_to_self() {
        let local = HypercubeAddress(0b101);
        let router = HypercubeRouter::new(local, 3);
        assert!(router.next_hop(local).is_none());
    }

    #[test]
    fn test_reject_non_neighbour() {
        let local = HypercubeAddress(0b000);
        let mut router = HypercubeRouter::new(local, 3);
        // Address 0b011 differs by 2 bits — not a direct neighbour.
        let entry = RouteEntry { address: HypercubeAddress(0b011), adnl_id: [0u8; 32] };
        assert!(!router.register_neighbour(entry));
    }
}
