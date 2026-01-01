use std::fs::File;
use std::io::Write;
use std::path::Path;
use memmap2::MmapOptions;
use rkyv::{to_bytes, check_archived_root};
use crate::models::{WorkspaceIndex, StagingArea, SymbolIndex};
use super::Indexer;

impl Indexer {
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        // Only save the Persistent Graph (WorkspaceIndex)
        // Staging (raw calls) and Lookup (caches) are discarded
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
        
        // Rebuild Lookup Index (Symbol Map)
        // Since we don't save the map, we must reconstruct it for CLI queries to work
        let mut lookup = SymbolIndex::default();
        for sym in index.symbols.values() {
             lookup.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);
        }

        let mut s = Self::new();
        s.index = index;
        s.lookup = lookup;
        s.staging = StagingArea::default(); // Staging is always empty on load
        
        // Rebuild the reverse graph from the persisted forward graph
        s.build_reverse_graph();
        
        Ok(s)
    }
}