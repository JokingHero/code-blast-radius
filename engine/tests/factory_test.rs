mod common;
use common::TestWorkspace;
use rfc_engine::resolution::Indexer;

#[test]
fn test_ts_factories() {
    let workspace = TestWorkspace::new();

    workspace.create_file("store.ts", r#"
        import { create } from 'zustand';

        // Should be detected as a function definition 'useStore'
        export const useStore = create((set) => ({
            bears: 0,
            increase: () => set((state) => ({ bears: state.bears + 1 })),
        }));
    "#);

    workspace.create_file("db.ts", r#"
        import mongoose from 'mongoose';

        // Should be detected as 'User'
        const User = mongoose.model('User', { name: String });
    "#);

    workspace.create_file("styles.tsx", r#"
        import styled from 'styled-components';

        // Should be detected as 'Title'
        const Title = styled.h1`
            font-size: 1.5em;
        `;
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    assert!(indexer.index.symbol_map.contains_key("useStore"), "Zustand create() pattern missed");
    assert!(indexer.index.symbol_map.contains_key("User"), "Mongoose model() pattern missed");
    assert!(indexer.index.symbol_map.contains_key("Title"), "Styled components pattern missed");
}