//! Correctness gate: before trusting any latency/size number, prove that every
//! exact-lookup engine resolves a word to the *same* `expr_index`, and that the
//! fuzzy oracle actually contains the words we typo'd. If this fails, the
//! comparison is meaningless.

use comparisons::*;
use wordtree::TreeFn;

#[test]
fn exact_lookup_engines_agree() {
    for lang in LANGS {
        let ds = Dataset::load(lang);
        assert!(ds.len() > 1000, "{lang}: suspiciously small dataset");

        let tree = build_wordtree(&ds.rows);
        let map = build_fst_map(&ds.rows);
        let hm = build_hashmap(&ds.rows);
        let sv = build_sorted_vec(&ds.rows);
        let bp = build_boomphf(&ds.rows);

        // Sample ~2000 words spread across the list.
        let step = (ds.len() / 2000).max(1);
        let mut checked = 0;
        for r in ds.rows.iter().step_by(step) {
            let expect = r.expr_index;
            assert_eq!(
                tree.index_of(&r.word),
                Some(expect),
                "{lang} wordtree {}",
                r.word
            );
            assert_eq!(
                map.get(&r.word),
                Some(expect as u64),
                "{lang} fst {}",
                r.word
            );
            assert_eq!(
                hm.get(&r.word).copied(),
                Some(expect),
                "{lang} hashmap {}",
                r.word
            );
            assert_eq!(
                sorted_vec_get(&sv, &r.word),
                Some(expect),
                "{lang} sorted-vec {}",
                r.word
            );
            assert_eq!(bp.get(&r.word), Some(expect), "{lang} boomphf {}", r.word);
            checked += 1;
        }
        assert!(checked > 100, "{lang}: too few samples checked");
        eprintln!(
            "{lang}: {} unique words, {checked} sampled, all engines agree",
            ds.len()
        );
    }
}

#[test]
fn fuzzy_oracle_contains_targets() {
    // The brute-force DL≤1 oracle must contain the word each typo was derived
    // from (a single edit means distance 1, so the target is always in the set).
    let ds = Dataset::load("sv");
    let queries = fuzzy_query_set(&ds, 25);
    assert!(!queries.is_empty(), "no fuzzy queries generated");
    for q in &queries {
        let truth = ground_truth_dl1(&ds, &q.text);
        assert!(
            truth.iter().any(|w| w == &q.target),
            "oracle for typo {:?} ({}) missing target {}",
            q.text,
            q.kind.label(),
            q.target
        );
    }
    eprintln!(
        "sv: {} fuzzy queries, oracle contains every target",
        queries.len()
    );
}

#[test]
fn wordtree_indel_asymmetry_isolated() {
    // Isolated reproduction of the README usage example (apple/apply/apricot),
    // removing the data pipeline entirely. Confirms wordtree's suggestion engine
    // corrects same-length edits but not length-changing ones (indels):
    //   - "appel"  (transpose l<->e of apple) -> finds "apple"
    //   - "applr"  (substitute e->r)          -> finds "apple"
    //   - "aple"   (delete a p; the README's OWN example string) -> MISSES "apple"
    //   - "appale" (insert an a)               -> MISSES "apple"
    let mut b = wordtree::Builder::new();
    b.add_word("apple", 99, 1);
    b.add_word("apply", 80, 2);
    b.add_word("apricot", 50, 3);
    b.organize_into_folders(100);
    let tree = b.to_tree();

    let words = |q: &str| -> Vec<String> {
        use wordtree::TreeFn;
        tree.suggestions(q, |_| true)
            .into_iter()
            .filter_map(|s| match s.expr_index {
                1 => Some("apple".to_string()),
                2 => Some("apply".to_string()),
                3 => Some("apricot".to_string()),
                _ => None,
            })
            .collect()
    };

    assert!(
        words("appel").contains(&"apple".to_string()),
        "transpose should correct"
    );
    assert!(
        words("applr").contains(&"apple".to_string()),
        "substitute should correct"
    );
    // Verified asymmetry — these are the indel cases:
    assert!(
        !words("aple").contains(&"apple".to_string()),
        "delete typo unexpectedly corrected"
    );
    assert!(
        !words("appale").contains(&"apple".to_string()),
        "insert typo unexpectedly corrected"
    );
    eprintln!("verified: wordtree corrects substitution/transposition but not indels");
}

#[test]
fn diagnose_typo_recall_by_kind() {
    // Diagnostic (not a hard gate): wordtree is a top-k frequency-ranked
    // suggester capped at ~3 spellings, so it is expected to recover the target
    // less often than exhaustive symspell. Break recall down by edit kind and
    // print a few concrete misses to confirm the behaviour is the cap, not a bug.
    let ds = Dataset::load("sv");
    let tree = build_wordtree(&ds.rows);
    let sym = build_symspell(&ds.rows);
    let queries = fuzzy_query_set(&ds, 25);

    for kind in EditKind::ALL {
        let qs: Vec<_> = queries.iter().filter(|q| q.kind == kind).collect();
        let mut wt = 0;
        let mut sy = 0;
        let mut shown = 0;
        for q in &qs {
            let wt_words = wordtree_suggest_words(&tree, &ds, &q.text);
            let sy_words = symspell_suggest_words(&sym, &q.text);
            let wt_hit = wt_words.iter().any(|w| w == &q.target);
            if wt_hit {
                wt += 1;
            }
            if sy_words.iter().any(|w| w == &q.target) {
                sy += 1;
            }
            if !wt_hit && shown < 2 {
                let truth = ground_truth_dl1(&ds, &q.text);
                let trank = rank_of(&truth, &q.target);
                eprintln!(
                    "  MISS [{}] typo {:?} target {:?} (pct {:?}) oracle#={} target_oracle_rank={:?}\n     wordtree returned: {:?}",
                    kind.label(),
                    q.text,
                    q.target,
                    ds.percentile_of(&q.target),
                    truth.len(),
                    trank,
                    wt_words,
                );
                shown += 1;
            }
        }
        let n = qs.len();
        eprintln!(
            "kind {:<11} wordtree {wt}/{n}  symspell {sy}/{n}",
            kind.label()
        );
    }
}
