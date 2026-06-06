//! A Kademlia-style Distributed Hash Table engine used to trace and locate
//! abstract node identities across the network.
//!
//! # Design
//! Each node maintains a routing table of `K_BUCKET_SIZE` contacts per bit
//! of the 256-bit key space, organised into 256 k-buckets. Lookup proceeds
//! by iteratively querying the `ALPHA` closest known nodes until the target
//! is found or no closer nodes can be discovered.
//!
//! Node distance is the standard Kademlia XOR metric over the 256-bit ADNL IDs.

use std::collections::{BTreeMap, VecDeque};

/// Maximum number of contacts stored per k-bucket.
pub const K_BUCKET_SIZE: usize = 20;

/// Concurrency parameter: number of parallel lookups per iteration.
pub const ALPHA: usize = 3;

/// A 256-bit node key (ADNL identity).
pub type NodeId = [u8; 32];

/// XOR distance between two 256-bit node IDs, represented as a big-endian
/// byte array for lexicographic ordering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance(pub [u8; 32]);

impl Distance {
    /// Computes the XOR distance between two node IDs.
    pub fn between(a: &NodeId, b: &NodeId) -> Self {
        let mut d = [0u8; 32];
        for i in 0..32 {
            d[i] = a[i] ^ b[i];
        }
        Distance(d)
    }

    /// Returns the index of the highest set bit (0-based from MSB), which
    /// determines which k-bucket this distance falls into.
    pub fn bucket_index(&self) -> Option<usize> {
        for (byte_idx, &byte) in self.0.iter().enumerate() {
            if byte != 0 {
                let bit_pos = byte.leading_zeros() as usize;
                return Some(byte_idx * 8 + bit_pos);
            }
        }
        None // distance is zero — same node
    }
}

/// A contact record stored in the DHT routing table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// The node's 256-bit ADNL identity.
    pub id: NodeId,
    /// An opaque network address string (e.g., "host:port").
    pub address: String,
}

/// A single k-bucket: a bounded FIFO queue of contacts ordered by last-seen time
/// (most recently seen at the back).
#[derive(Debug, Default)]
pub struct KBucket {
    contacts: VecDeque<Contact>,
}

impl KBucket {
    /// Inserts or refreshes a contact in the bucket.
    ///
    /// - If the contact already exists, it is moved to the back (most recent).
    /// - If the bucket is not full, the contact is appended to the back.
    /// - If the bucket is full, the contact is dropped (the oldest contact at
    ///   the front should be probed for liveness before eviction — simplified
    ///   here by simply discarding the new contact).
    pub fn update(&mut self, contact: Contact) {
        // Remove existing entry if present.
        if let Some(pos) = self.contacts.iter().position(|c| c.id == contact.id) {
            self.contacts.remove(pos);
        }
        if self.contacts.len() < K_BUCKET_SIZE {
            self.contacts.push_back(contact);
        }
        // If full, drop the new contact (oldest survives — standard Kademlia behaviour).
    }

    /// Returns up to `count` contacts from this bucket, ordered most-recent first.
    pub fn closest(&self, count: usize) -> Vec<&Contact> {
        self.contacts.iter().rev().take(count).collect()
    }

    /// Returns the number of contacts in this bucket.
    pub fn len(&self) -> usize {
        self.contacts.len()
    }

    /// Returns `true` if the bucket contains no contacts.
    pub fn is_empty(&self) -> bool {
        self.contacts.is_empty()
    }
}

/// The full DHT routing table for a single local node.
///
/// Contains 256 k-buckets, one per bit of the key space.
pub struct RoutingTable {
    /// This node's own ID.
    pub local_id: NodeId,
    /// 256 k-buckets indexed by XOR distance bucket index.
    buckets: Vec<KBucket>,
}

impl RoutingTable {
    /// Creates a new, empty routing table for the given local node ID.
    pub fn new(local_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(256);
        for _ in 0..256 {
            buckets.push(KBucket::default());
        }
        Self { local_id, buckets }
    }

    /// Inserts or refreshes a contact. Ignores the local node itself.
    pub fn update(&mut self, contact: Contact) {
        if contact.id == self.local_id {
            return;
        }
        let dist = Distance::between(&self.local_id, &contact.id);
        if let Some(idx) = dist.bucket_index() {
            self.buckets[idx].update(contact);
        }
    }

    /// Returns the `k` contacts closest to the given target ID, sorted by
    /// ascending XOR distance.
    pub fn closest_to(&self, target: &NodeId, k: usize) -> Vec<&Contact> {
        // Collect all contacts from all buckets.
        let mut all: Vec<(&Contact, Distance)> = self
            .buckets
            .iter()
            .flat_map(|b| b.contacts.iter())
            .map(|c| {
                let d = Distance::between(target, &c.id);
                (c, d)
            })
            .collect();

        // Sort by ascending distance.
        all.sort_by(|a, b| a.1.cmp(&b.1));
        all.into_iter().take(k).map(|(c, _)| c).collect()
    }

    /// Returns the total number of contacts across all buckets.
    pub fn total_contacts(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }
}

/// Simulates a single iterative Kademlia `FIND_NODE` lookup.
///
/// Given a routing table and a target ID, returns the closest known contacts
/// up to `K_BUCKET_SIZE`. In a real implementation this would involve multiple
/// async RPC rounds; here we perform a single-pass closest-node selection.
pub fn find_node<'a>(table: &'a RoutingTable, target: &NodeId) -> Vec<&'a Contact> {
    table.closest_to(target, K_BUCKET_SIZE)
}

/// A lightweight DHT node combining a local identity with its routing table.
pub struct DhtNode {
    pub id: NodeId,
    pub routing_table: RoutingTable,
}

impl DhtNode {
    /// Creates a new DHT node with the given ADNL identity.
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            routing_table: RoutingTable::new(id),
        }
    }

    /// Bootstraps the node by inserting a list of known seed contacts.
    pub fn bootstrap(&mut self, seeds: Vec<Contact>) {
        for seed in seeds {
            self.routing_table.update(seed);
        }
    }

    /// Looks up the closest known contacts to the given target.
    pub fn lookup(&self, target: &NodeId) -> Vec<&Contact> {
        find_node(&self.routing_table, target)
    }
}

// ─── Helpers for BTreeMap-based sorted contact sets (used in iterative lookup) ──

/// Builds a `BTreeMap<Distance, Contact>` of the closest contacts to a target,
/// useful for maintaining the sorted shortlist during iterative lookup rounds.
pub fn sorted_shortlist(
    contacts: Vec<Contact>,
    target: &NodeId,
    limit: usize,
) -> BTreeMap<Distance, Contact> {
    let mut map = BTreeMap::new();
    for c in contacts {
        let d = Distance::between(target, &c.id);
        map.insert(d, c);
        if map.len() > limit {
            // Remove the farthest entry.
            let last_key = map.keys().next_back().cloned().unwrap();
            map.remove(&last_key);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id(byte: u8) -> NodeId {
        [byte; 32]
    }

    fn make_contact(byte: u8) -> Contact {
        Contact {
            id: make_id(byte),
            address: format!("127.0.0.{byte}:8080"),
        }
    }

    #[test]
    fn test_distance_zero_for_same_id() {
        let id = make_id(42);
        let d = Distance::between(&id, &id);
        assert_eq!(d.0, [0u8; 32]);
        assert!(d.bucket_index().is_none());
    }

    #[test]
    fn test_distance_bucket_index() {
        let a = [0u8; 32];
        let mut b = [0u8; 32];
        b[0] = 0b0000_0001; // differs in bit 7 of byte 0 → bucket index 7
        let d = Distance::between(&a, &b);
        assert_eq!(d.bucket_index(), Some(7));
    }

    #[test]
    fn test_kbucket_update_and_refresh() {
        let mut bucket = KBucket::default();
        let c = make_contact(1);
        bucket.update(c.clone());
        bucket.update(c.clone()); // refresh
        assert_eq!(bucket.len(), 1);
    }

    #[test]
    fn test_kbucket_capacity() {
        let mut bucket = KBucket::default();
        for i in 0..=K_BUCKET_SIZE as u8 {
            bucket.update(make_contact(i));
        }
        // Should not exceed K_BUCKET_SIZE.
        assert_eq!(bucket.len(), K_BUCKET_SIZE);
    }

    #[test]
    fn test_routing_table_closest() {
        let local = make_id(0);
        let mut table = RoutingTable::new(local);
        for i in 1..=10u8 {
            table.update(make_contact(i));
        }
        let target = make_id(3);
        let closest = table.closest_to(&target, 3);
        assert_eq!(closest.len(), 3);
        // Verify sorted by ascending distance.
        let dists: Vec<Distance> = closest
            .iter()
            .map(|c| Distance::between(&target, &c.id))
            .collect();
        for w in dists.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }

    #[test]
    fn test_dht_node_bootstrap_and_lookup() {
        let mut node = DhtNode::new(make_id(0));
        let seeds: Vec<Contact> = (1..=5u8).map(make_contact).collect();
        node.bootstrap(seeds);
        assert_eq!(node.routing_table.total_contacts(), 5);
        let results = node.lookup(&make_id(3));
        assert!(!results.is_empty());
    }

    #[test]
    fn test_sorted_shortlist() {
        let target = make_id(0);
        let contacts: Vec<Contact> = (1..=10u8).map(make_contact).collect();
        let list = sorted_shortlist(contacts, &target, 3);
        assert_eq!(list.len(), 3);
    }
}
