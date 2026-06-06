//! Provides a performance-tuned embedded key-value wrapper designed specifically
//! for atomic Bag-of-Cells storage reads and Merkle path indexing.
//!
//! # Design
//! The storage layer abstracts over an underlying KV engine (production: RocksDB;
//! testing: in-memory `BTreeMap`). All cell hashes are 32-byte keys; values are
//! raw serialised cell bytes. Column families separate:
//!
//! - `CF_CELLS`  — raw cell data keyed by cell hash
//! - `CF_BLOCKS` — block header bytes keyed by block hash
//! - `CF_META`   — miscellaneous metadata (e.g., chain tip, genesis hash)
//!
//! Write batches guarantee atomicity: either all cells in a BoC root are written
//! together, or none are.

use std::collections::HashMap;

// ─── Column family names ──────────────────────────────────────────────────────

pub const CF_CELLS: &str = "cells";
pub const CF_BLOCKS: &str = "blocks";
pub const CF_META: &str = "meta";

/// Well-known metadata keys.
pub mod meta_keys {
    pub const CHAIN_TIP: &[u8] = b"chain_tip";
    pub const GENESIS_HASH: &[u8] = b"genesis_hash";
    pub const BLOCK_HEIGHT: &[u8] = b"block_height";
}

// ─── Error type ──────────────────────────────────────────────────────────────

/// Errors that can arise from storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbError {
    /// The requested key was not found.
    NotFound,
    /// A write batch was applied to an unknown column family.
    UnknownColumnFamily(String),
    /// The database is in a read-only state and cannot accept writes.
    ReadOnly,
    /// Generic I/O or serialisation error (message string).
    Io(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::NotFound => write!(f, "key not found"),
            DbError::UnknownColumnFamily(cf) => write!(f, "unknown column family: {cf}"),
            DbError::ReadOnly => write!(f, "database is read-only"),
            DbError::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

pub type DbResult<T> = Result<T, DbError>;

// ─── Write batch ─────────────────────────────────────────────────────────────

/// A single operation within a write batch.
#[derive(Debug, Clone)]
pub enum BatchOp {
    Put {
        cf: String,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Delete {
        cf: String,
        key: Vec<u8>,
    },
}

/// An atomic collection of put/delete operations applied together.
#[derive(Debug, Default, Clone)]
pub struct WriteBatch {
    ops: Vec<BatchOp>,
}

impl WriteBatch {
    /// Creates an empty write batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a put operation.
    pub fn put(&mut self, cf: &str, key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) {
        self.ops.push(BatchOp::Put {
            cf: cf.to_string(),
            key: key.into(),
            value: value.into(),
        });
    }

    /// Queues a delete operation.
    pub fn delete(&mut self, cf: &str, key: impl Into<Vec<u8>>) {
        self.ops.push(BatchOp::Delete {
            cf: cf.to_string(),
            key: key.into(),
        });
    }

    /// Returns the number of operations in this batch.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns `true` if the batch contains no operations.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

// ─── In-memory database (used for testing and simulation) ────────────────────

/// An in-memory key-value store that mirrors the RocksDB column-family API.
/// Suitable for unit tests and the simulation crate; swap for a RocksDB
/// backend in production by implementing the same `KvStore` trait.
#[derive(Debug, Default)]
pub struct MemDb {
    /// Column families: cf_name → (key → value).
    data: HashMap<String, HashMap<Vec<u8>, Vec<u8>>>,
    read_only: bool,
}

impl MemDb {
    /// Creates a new, empty in-memory database with the standard column families.
    pub fn open() -> Self {
        let mut db = Self::default();
        db.data.insert(CF_CELLS.to_string(), HashMap::new());
        db.data.insert(CF_BLOCKS.to_string(), HashMap::new());
        db.data.insert(CF_META.to_string(), HashMap::new());
        db
    }

    /// Creates a read-only view (writes will return `DbError::ReadOnly`).
    pub fn open_read_only() -> Self {
        let mut db = Self::open();
        db.read_only = true;
        db
    }

    /// Reads a value from the given column family.
    pub fn get(&self, cf: &str, key: &[u8]) -> DbResult<Vec<u8>> {
        let cf_map = self
            .data
            .get(cf)
            .ok_or_else(|| DbError::UnknownColumnFamily(cf.to_string()))?;
        cf_map
            .get(key)
            .cloned()
            .ok_or(DbError::NotFound)
    }

    /// Writes a single key-value pair to the given column family.
    pub fn put(&mut self, cf: &str, key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> DbResult<()> {
        if self.read_only {
            return Err(DbError::ReadOnly);
        }
        let cf_map = self
            .data
            .get_mut(cf)
            .ok_or_else(|| DbError::UnknownColumnFamily(cf.to_string()))?;
        cf_map.insert(key.into(), value.into());
        Ok(())
    }

    /// Deletes a key from the given column family. Returns `Ok(())` even if
    /// the key did not exist (idempotent).
    pub fn delete(&mut self, cf: &str, key: &[u8]) -> DbResult<()> {
        if self.read_only {
            return Err(DbError::ReadOnly);
        }
        let cf_map = self
            .data
            .get_mut(cf)
            .ok_or_else(|| DbError::UnknownColumnFamily(cf.to_string()))?;
        cf_map.remove(key);
        Ok(())
    }

    /// Applies a `WriteBatch` atomically. If any operation references an unknown
    /// column family the entire batch is rolled back.
    pub fn write_batch(&mut self, batch: WriteBatch) -> DbResult<()> {
        if self.read_only {
            return Err(DbError::ReadOnly);
        }
        // Validate all ops before applying (simulate atomicity).
        for op in &batch.ops {
            let cf = match op {
                BatchOp::Put { cf, .. } | BatchOp::Delete { cf, .. } => cf,
            };
            if !self.data.contains_key(cf.as_str()) {
                return Err(DbError::UnknownColumnFamily(cf.clone()));
            }
        }
        // Apply.
        for op in batch.ops {
            match op {
                BatchOp::Put { cf, key, value } => {
                    self.data.get_mut(&cf).unwrap().insert(key, value);
                }
                BatchOp::Delete { cf, key } => {
                    self.data.get_mut(&cf).unwrap().remove(&key);
                }
            }
        }
        Ok(())
    }

    /// Returns `true` if the given key exists in the column family.
    pub fn exists(&self, cf: &str, key: &[u8]) -> bool {
        self.data
            .get(cf)
            .map(|m| m.contains_key(key))
            .unwrap_or(false)
    }

    /// Returns the number of entries in the given column family.
    pub fn count(&self, cf: &str) -> usize {
        self.data.get(cf).map(|m| m.len()).unwrap_or(0)
    }

    // ── Convenience helpers for BoC cell storage ──────────────────────────

    /// Stores a raw cell blob keyed by its 32-byte hash.
    pub fn put_cell(&mut self, hash: &[u8; 32], data: &[u8]) -> DbResult<()> {
        self.put(CF_CELLS, hash.as_ref(), data)
    }

    /// Retrieves a raw cell blob by its 32-byte hash.
    pub fn get_cell(&self, hash: &[u8; 32]) -> DbResult<Vec<u8>> {
        self.get(CF_CELLS, hash.as_ref())
    }

    /// Stores a raw block header blob keyed by its 32-byte hash.
    pub fn put_block(&mut self, hash: &[u8; 32], data: &[u8]) -> DbResult<()> {
        self.put(CF_BLOCKS, hash.as_ref(), data)
    }

    /// Retrieves a raw block header blob by its 32-byte hash.
    pub fn get_block(&self, hash: &[u8; 32]) -> DbResult<Vec<u8>> {
        self.get(CF_BLOCKS, hash.as_ref())
    }

    /// Writes a metadata value.
    pub fn put_meta(&mut self, key: &[u8], value: &[u8]) -> DbResult<()> {
        self.put(CF_META, key, value)
    }

    /// Reads a metadata value.
    pub fn get_meta(&self, key: &[u8]) -> DbResult<Vec<u8>> {
        self.get(CF_META, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_and_get() {
        let mut db = MemDb::open();
        db.put(CF_CELLS, b"key1".to_vec(), b"value1".to_vec()).unwrap();
        let v = db.get(CF_CELLS, b"key1").unwrap();
        assert_eq!(v, b"value1");
    }

    #[test]
    fn test_not_found() {
        let db = MemDb::open();
        assert_eq!(db.get(CF_CELLS, b"missing"), Err(DbError::NotFound));
    }

    #[test]
    fn test_delete() {
        let mut db = MemDb::open();
        db.put(CF_CELLS, b"k".to_vec(), b"v".to_vec()).unwrap();
        db.delete(CF_CELLS, b"k").unwrap();
        assert_eq!(db.get(CF_CELLS, b"k"), Err(DbError::NotFound));
    }

    #[test]
    fn test_write_batch_atomic() {
        let mut db = MemDb::open();
        let mut batch = WriteBatch::new();
        batch.put(CF_CELLS, b"a".to_vec(), b"1".to_vec());
        batch.put(CF_BLOCKS, b"b".to_vec(), b"2".to_vec());
        db.write_batch(batch).unwrap();
        assert_eq!(db.get(CF_CELLS, b"a").unwrap(), b"1");
        assert_eq!(db.get(CF_BLOCKS, b"b").unwrap(), b"2");
    }

    #[test]
    fn test_write_batch_rollback_on_bad_cf() {
        let mut db = MemDb::open();
        let mut batch = WriteBatch::new();
        batch.put(CF_CELLS, b"good".to_vec(), b"ok".to_vec());
        batch.put("nonexistent_cf", b"bad".to_vec(), b"fail".to_vec());
        let result = db.write_batch(batch);
        assert!(result.is_err());
        // The good key should NOT have been written (rollback).
        assert_eq!(db.get(CF_CELLS, b"good"), Err(DbError::NotFound));
    }

    #[test]
    fn test_read_only_rejects_writes() {
        let mut db = MemDb::open_read_only();
        let result = db.put(CF_CELLS, b"k".to_vec(), b"v".to_vec());
        assert_eq!(result, Err(DbError::ReadOnly));
    }

    #[test]
    fn test_cell_convenience_helpers() {
        let mut db = MemDb::open();
        let hash = [0xABu8; 32];
        let data = vec![1, 2, 3, 4];
        db.put_cell(&hash, &data).unwrap();
        assert_eq!(db.get_cell(&hash).unwrap(), data);
    }

    #[test]
    fn test_meta_helpers() {
        let mut db = MemDb::open();
        db.put_meta(meta_keys::BLOCK_HEIGHT, b"42").unwrap();
        assert_eq!(db.get_meta(meta_keys::BLOCK_HEIGHT).unwrap(), b"42");
    }
}
