use std::fs::File;
use std::io::Write;
use std::path::Path;
use memmap2::MmapOptions;
use rkyv::{to_bytes, check_archived_root};
use crate::models::WorkspaceIndex;
use super::Indexer;

impl Indexer {
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let bytes = to_bytes::<_, 4096>(&self.index).map_err(|e|
            anyhow::anyhow!("Serialization failed: {}", e)
        )?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        
        if let Err(e) = check_archived_root::<WorkspaceIndex>(&mmap[..]) {
            eprintln!("Index corrupted: {}", e);
            return Ok(Self::new());
        }
        
        let index: WorkspaceIndex = unsafe {
            rkyv::from_bytes_unchecked(&mmap[..]).map_err(|e| anyhow::anyhow!(e))?
        };
        
        let mut s = Self::new();
        s.index = index;
        Ok(s)
    }
}