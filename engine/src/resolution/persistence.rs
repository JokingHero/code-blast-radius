use crate::models::BoundaryIndex;
use anyhow::{Context, Result};
use memmap2::MmapOptions;
use rkyv::{check_archived_root, to_bytes};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Handles the serialization and deserialization of the simplified BoundaryIndex.
pub struct PersistenceManager;

impl PersistenceManager {
    pub fn new() -> Self {
        Self
    }

    /// Serializes the BoundaryIndex to the specified path using rkyv.
    /// This is extremely fast and zero-copy friendly.
    pub fn save_index(&self, index: &BoundaryIndex, path: &Path) -> Result<()> {
        let bytes = to_bytes::<_, 4096>(index)
            .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = File::create(path).context("Failed to create index file")?;
        file.write_all(&bytes)
            .context("Failed to write index bytes")?;
        Ok(())
    }

    /// Loads the BoundaryIndex from the specified path.
    pub fn load_index(&self, path: &Path) -> Result<BoundaryIndex> {
        if !path.exists() {
            return Ok(BoundaryIndex::new());
        }

        let file = File::open(path).context("Failed to open index file")?;

        // Safety: We assume the file is not modified externally while mapped.
        // For a single-user GUI/CLI tool, this is an acceptable tradeoff for performance.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        if let Err(e) = check_archived_root::<BoundaryIndex>(&mmap[..]) {
            eprintln!("Index corrupted or version mismatch: {}", e);
            return Ok(BoundaryIndex::new());
        }

        // Deserialize fully into memory.
        // While rkyv supports zero-copy access, we want a mutable HashMap
        // to update the index during scans, so we deserialize to owned types.
        let index: BoundaryIndex =
            unsafe { rkyv::from_bytes_unchecked(&mmap[..]).map_err(|e| anyhow::anyhow!(e))? };

        Ok(index)
    }
}
