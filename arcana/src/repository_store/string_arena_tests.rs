use super::*;

#[test]
fn interning_deduplicates_without_copying_duplicate_bytes() {
    let mut arena = StagedStringArena::default();
    let first = arena.intern("alpha").unwrap();
    let second = arena.intern("alpha").unwrap();
    let third = arena.intern("beta").unwrap();

    assert_eq!(first, second);
    assert_ne!(first, third);
    assert_eq!(arena.len(), 2);
    assert_eq!(arena.byte_len(), "alpha".len() + "beta".len());
}

#[test]
fn freeze_orders_lexically_and_remaps_live_handles() {
    let mut arena = StagedStringArena::default();
    let gamma = arena.intern("gamma").unwrap();
    let alpha = arena.intern("alpha").unwrap();
    let unused = arena.intern("discard").unwrap();
    let beta = arena.intern("beta").unwrap();

    let mut used = vec![false; arena.len()];
    for id in [gamma, alpha, beta] {
        used[id.0 as usize] = true;
    }

    let (table, remap) = arena.freeze(&used).unwrap();

    assert_eq!(
        table.values().collect::<Vec<_>>(),
        vec!["alpha", "beta", "gamma"]
    );
    assert_eq!(remap[alpha.0 as usize], StringId(0));
    assert_eq!(remap[beta.0 as usize], StringId(1));
    assert_eq!(remap[gamma.0 as usize], StringId(2));
    assert_eq!(remap[unused.0 as usize], StringId::ABSENT);
}

#[test]
fn freeze_is_deterministic_across_insertion_orders() {
    fn table(order: &[&str]) -> CompactStringTable {
        let mut arena = StagedStringArena::default();
        for value in order {
            arena.intern(value).unwrap();
        }
        let used = vec![true; arena.len()];
        arena.freeze(&used).unwrap().0
    }

    let forward = table(&["zeta", "alpha", "middle"]);
    let reverse = table(&["middle", "alpha", "zeta"]);

    assert_eq!(forward, reverse);
    assert_eq!(forward.encode().unwrap(), reverse.encode().unwrap());
}

#[test]
fn freeze_preserves_controls_utf8_and_exact_binary_bytes() {
    let mut arena = StagedStringArena::default();
    for value in ["☃", "line\n", "a\0b", "", "tab\t"] {
        arena.intern(value).unwrap();
    }
    let used = vec![true; arena.len()];
    let (table, _) = arena.freeze(&used).unwrap();

    assert_eq!(
        table.values().collect::<Vec<_>>(),
        vec!["", "a\0b", "line\n", "tab\t", "☃"]
    );

    let expected_values = ["", "a\0b", "line\n", "tab\t", "☃"];
    let mut expected = Vec::new();
    let mut offset = 0_u64;
    for value in expected_values {
        expected.extend_from_slice(&offset.to_le_bytes());
        expected.extend_from_slice(&(value.len() as u32).to_le_bytes());
        expected.extend_from_slice(&0_u32.to_le_bytes());
        offset += value.len() as u64;
    }
    for value in expected_values {
        expected.extend_from_slice(value.as_bytes());
    }

    assert_eq!(table.encode().unwrap(), expected);
    let decoded = CompactStringTable::decode(&expected, expected_values.len() as u64).unwrap();
    assert_eq!(decoded, table);
}
