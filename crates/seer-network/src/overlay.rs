//! Orchestrates dynamic multicast overlay meshes to segment ledger streaming
//! into performance-optimized gossip branches.

use std::collections::{HashMap, HashSet};

/// A unique identifier for an overlay network, derived from its purpose/shard ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayId(pub [u8; 32]);

/// Represents a single peer within an overlay mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayPeer {
    /// The peer's 256-bit ADNL abstract identity.
    pub adnl_id: [u8; 32],
}

/// The type of message being broadcast across an overlay.
#[derive(Debug, Clone)]
pub enum OverlayMessage {
    /// A new transaction announcement.
    Transaction(Vec<u8>),
    /// A new block announcement.
    Block(Vec<u8>),
    /// A vertical patch propagation.
    Patch(Vec<u8>),
    /// A generic inventory advertisement (hash only).
    Inventory([u8; 32]),
}

/// Manages a single overlay mesh: its membership and gossip fanout.
#[derive(Debug)]
pub struct OverlayMesh {
    /// The unique identifier for this overlay.
    pub id: OverlayId,
    /// The set of peers currently participating in this overlay.
    peers: HashSet<OverlayPeer>,
    /// Maximum number of peers to forward a gossip message to (fanout).
    fanout: usize,
}

impl OverlayMesh {
    /// Creates a new overlay mesh with the given ID and gossip fanout.
    pub fn new(id: OverlayId, fanout: usize) -> Self {
        Self {
            id,
            peers: HashSet::new(),
            fanout,
        }
    }

    /// Adds a peer to the overlay mesh.
    pub fn add_peer(&mut self, peer: OverlayPeer) {
        self.peers.insert(peer);
    }

    /// Removes a peer from the overlay mesh.
    pub fn remove_peer(&mut self, peer: &OverlayPeer) {
        self.peers.remove(peer);
    }

    /// Returns the current number of peers in this overlay.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Selects up to `fanout` peers to forward a gossip message to,
    /// excluding the originating peer.
    pub fn select_gossip_targets(&self, origin: &OverlayPeer) -> Vec<&OverlayPeer> {
        self.peers
            .iter()
            .filter(|p| *p != origin)
            .take(self.fanout)
            .collect()
    }

    /// Simulates broadcasting a message to all selected gossip targets.
    /// Returns the list of peers that would receive the message.
    pub fn broadcast(&self, origin: &OverlayPeer, msg: &OverlayMessage) -> Vec<&OverlayPeer> {
        let targets = self.select_gossip_targets(origin);
        let _ = msg;
        targets
    }
}

/// The top-level overlay manager, maintaining multiple named overlay meshes.
#[derive(Debug, Default)]
pub struct OverlayManager {
    meshes: HashMap<OverlayId, OverlayMesh>,
}

impl OverlayManager {
    /// Creates a new, empty overlay manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new overlay mesh under the given ID.
    pub fn register(&mut self, id: OverlayId, fanout: usize) {
        self.meshes
            .entry(id.clone())
            .or_insert_with(|| OverlayMesh::new(id, fanout));
    }

    /// Returns a mutable reference to the mesh for the given overlay ID.
    pub fn get_mut(&mut self, id: &OverlayId) -> Option<&mut OverlayMesh> {
        self.meshes.get_mut(id)
    }

    /// Returns an immutable reference to the mesh for the given overlay ID.
    pub fn get(&self, id: &OverlayId) -> Option<&OverlayMesh> {
        self.meshes.get(id)
    }

    /// Removes and deregisters an overlay mesh.
    pub fn deregister(&mut self, id: &OverlayId) {
        self.meshes.remove(id);
    }

    /// Returns the number of active overlay meshes.
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(byte: u8) -> OverlayPeer {
        OverlayPeer { adnl_id: [byte; 32] }
    }

    #[test]
    fn test_overlay_add_remove_peer() {
        let id = OverlayId([0u8; 32]);
        let mut mesh = OverlayMesh::new(id, 3);
        let p1 = make_peer(1);
        let p2 = make_peer(2);
        mesh.add_peer(p1.clone());
        mesh.add_peer(p2.clone());
        assert_eq!(mesh.peer_count(), 2);
        mesh.remove_peer(&p1);
        assert_eq!(mesh.peer_count(), 1);
    }

    #[test]
    fn test_gossip_excludes_origin() {
        let id = OverlayId([0u8; 32]);
        let mut mesh = OverlayMesh::new(id, 10);
        let origin = make_peer(0);
        for i in 1..=5u8 {
            mesh.add_peer(make_peer(i));
        }
        mesh.add_peer(origin.clone());
        let targets = mesh.select_gossip_targets(&origin);
        assert!(!targets.contains(&&origin));
        assert_eq!(targets.len(), 5);
    }

    #[test]
    fn test_manager_register_deregister() {
        let mut mgr = OverlayManager::new();
        let id = OverlayId([1u8; 32]);
        mgr.register(id.clone(), 4);
        assert_eq!(mgr.mesh_count(), 1);
        mgr.deregister(&id);
        assert_eq!(mgr.mesh_count(), 0);
    }
}
