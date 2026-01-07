use std::fs::File;
use std::io::Write;
use std::path::Path;
use memmap2::MmapOptions;
use rkyv::{ to_bytes, check_archived_root };
use anyhow::{ Result, Context };
use crate::models::WorkspaceIndex;
/// Handles the serialization and deserialization of the Knowledge Graph.
/// It knows nothing about "Resolution", "Staging", or "Lookups".
pub struct PersistenceManager;
impl PersistenceManager {
    pub fn new() -> Self {
        Self
    }
    /// Serializes the WorkspaceIndex to the specified path using rkyv.
    pub fn save_index(&self, index: &WorkspaceIndex, path: &Path) -> Result<()> {
        // rkyv will serialize the new Logical Keys and FileNode relative paths automatically
        let bytes = to_bytes::<_, 4096>(index).map_err(|e|
            anyhow::anyhow!("Serialization failed: {}", e)
        )?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = File::create(path).context("Failed to create index file")?;
        file.write_all(&bytes).context("Failed to write index bytes")?;
        Ok(())
    }

    /// Loads the WorkspaceIndex from the specified path.
    /// Returns a raw WorkspaceIndex. It does NOT rebuild lookups (that is the domain logic's job).
    pub fn load_index(&self, path: &Path) -> Result<WorkspaceIndex> {
        if !path.exists() {
            // If file doesn't exist, return a default empty index
            return Ok(WorkspaceIndex::default());
        }

        let file = File::open(path).context("Failed to open index file")?;

        // Safety: Mmap is unsafe because external processes modifying the file
        // can cause UB. In this tool's context, it's generally acceptable.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        if let Err(e) = check_archived_root::<WorkspaceIndex>(&mmap[..]) {
            // If corrupted, return default rather than crashing, but log the error context
            eprintln!("Index corrupted or version mismatch: {}", e);
            return Ok(WorkspaceIndex::default());
        }

        let index: WorkspaceIndex = unsafe {
            rkyv::from_bytes_unchecked(&mmap[..]).map_err(|e| anyhow::anyhow!(e))?
        };

        Ok(index)
    }
}