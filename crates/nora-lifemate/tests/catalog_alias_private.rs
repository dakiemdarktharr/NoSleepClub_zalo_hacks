//! Private catalog property tests; this file is not required by the patch.
use nora_lifemate::{
    aliases::NameMap,
    protocol::{Catalog, ToolCall},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
#[test]
#[ignore = "requires local official catalogs; no samples"]
fn both_official_catalogs_keep_every_schema_and_match_independent_reference_hash() {
    let root = PathBuf::from(
        std::env::var("NORA_ALIAS_TEST_CATALOG_ROOT").expect("private catalog test root"),
    );
    for (split, count, changed, expected) in [
        (
            "train",
            147,
            39,
            "5dc8f51e5fb0d6b2bb3133d2c3ae60256ec2e026b8e6bab8f3e48ad679fb887f",
        ),
        (
            "public_test",
            122,
            34,
            "1237284f0556f09c50dc1611b66f311a59ad1acce40beca81dc2445f63d07443",
        ),
    ] {
        let bytes =
            std::fs::read(root.join(split).join(format!("{split}_tools_catalog.json"))).unwrap();
        let c: Catalog = serde_json::from_slice(&bytes).unwrap();
        let names: Vec<_> = c.apps.iter().flat_map(|a| a.tools.iter()).collect();
        let map = NameMap::from_catalog(&c).unwrap();
        assert_eq!(names.len(), count);
        let mut pairs = BTreeMap::new();
        let mut n_changed = 0;
        for original in names {
            let alias = map
                .model_tools(std::slice::from_ref(original))
                .unwrap()
                .pop()
                .unwrap();
            if alias.name != original.name {
                n_changed += 1;
            }
            assert_eq!(alias.parameters, original.parameters);
            assert_eq!(alias.returns, original.returns);
            assert_eq!(alias.description, original.description);
            assert_eq!(alias.kind, original.kind);
            let official = ToolCall {
                name: original.name.clone(),
                arguments: json!({}),
            };
            assert_eq!(
                map.sdk_call(
                    &map.model_call(&official).unwrap(),
                    std::slice::from_ref(original)
                )
                .unwrap(),
                official
            );
            pairs.insert(original.name.clone(), alias.name);
        }
        assert_eq!(n_changed, changed);
        let canonical = serde_json::to_vec(&pairs).unwrap();
        let actual = format!("{:x}", Sha256::digest(canonical));
        assert_eq!(actual, expected);
    }
}
