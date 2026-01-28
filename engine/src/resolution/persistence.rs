use crate::models::BoundaryIndex;
use anyhow::{Context, Result};
use memmap2::MmapOptions;
use rkyv::{check_archived_root, to_bytes};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct PersistenceManager;

impl PersistenceManager {
    pub fn new() -> Self {
        Self
    }

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

    pub fn load_index(&self, path: &Path) -> Result<BoundaryIndex> {
        if !path.exists() {
            return Ok(BoundaryIndex::new());
        }

        let file = File::open(path).context("Failed to open index file")?;
        
        // Safety: We assume single-user access lock via OS is unnecessary for this tool context
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let archived = match check_archived_root::<BoundaryIndex>(&mmap[..]) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Index corrupted or schema mismatch: {}", e);
                return Ok(BoundaryIndex::new());
            }
        };

        let current_version = env!("CARGO_PKG_VERSION");
        
        // Access the archived field directly without full deserialization first
        if archived.app_version.as_str() != current_version {
            eprintln!(
                "Index version mismatch (Disk: {}, App: {}). Rebuilding index...", 
                archived.app_version.as_str(), 
                current_version
            );
            return Ok(BoundaryIndex::new());
        }

        let index: BoundaryIndex =
            unsafe { rkyv::from_bytes_unchecked(&mmap[..]).map_err(|e| anyhow::anyhow!(e))? };

        Ok(index)
    }
}