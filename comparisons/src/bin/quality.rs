//! Suggestion quality. Latency is meaningless without knowing whether the
//! engines return the *right* words, so this scores each against a brute-force
//! Damerau-Levenshtein ≤1 oracle.
//!
//! Three views per language:
//!   1. Typo correction recall by edit kind — exposes wordtree's indel gap and
//!      fst's missing transposition (plain Levenshtein).
//!   2. Suggestion-set shape — wordtree is a small top-k suggester; symspell is
//!      exhaustive. Avg result count + avg recall of the full oracle.
//!   3. Autocomplete — top-5 agreement with the frequency oracle.

use comparisons::*;
use wordtree::{Caps, TreeFn};

const PER_KIND: usize = 50;
const PREFIXES: usize = 25;

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn main() {
    for lang in LANGS {
        let ds = Dataset::load(lang);
        let tree = build_wordtree(&ds.rows);
        let sym = build_symspell(&ds.rows);
        let map = build_fst_map(&ds.rows);

        let queries = fuzzy_query_set(&ds, PER_KIND);

        // Adapter table: name -> word list for a query.
        type SuggestFn<'a> = &'a dyn Fn(&str) -> Vec<String>;
        let engines: [(&str, SuggestFn); 3] = [
            ("wordtree", &|q: &str| wordtree_suggest_words(&tree, &ds, q)),
            ("symspell", &|q: &str| symspell_suggest_words(&sym, q)),
            ("fst-lev", &|q: &str| fst_suggest_words(&map, &ds, q)),
        ];

        println!("## {lang}\n");

        // --- 1. Typo correction recall by edit kind ---------------------------
        println!("### Typo correction — recall of the intended word, by edit kind\n");
        println!("| engine | substitute | transpose | delete | insert | overall |");
        println!("| ------ | ---------: | --------: | -----: | -----: | ------: |");
        for (name, f) in &engines {
            let mut cells = Vec::new();
            let mut overall = Vec::new();
            for kind in [
                EditKind::Substitute,
                EditKind::Transpose,
                EditKind::Delete,
                EditKind::Insert,
            ] {
                let hits: Vec<f64> = queries
                    .iter()
                    .filter(|q| q.kind == kind)
                    .map(|q| {
                        let got = f(&q.text);
                        if got.iter().any(|w| w == &q.target) {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect();
                overall.extend(hits.iter().copied());
                cells.push(pct(mean(&hits)));
            }
            println!(
                "| {name} | {} | {} | {} | {} | {} |",
                cells[0],
                cells[1],
                cells[2],
                cells[3],
                pct(mean(&overall))
            );
        }
        println!("\n*(brute-force DL≤1 = 100% by definition — it is the oracle.)*\n");

        // --- 2. Suggestion-set shape -----------------------------------------
        println!(
            "### Suggestion-set shape (avg over all {} typo queries)\n",
            queries.len()
        );
        println!("| engine | avg results returned | avg recall of full DL≤1 set |");
        println!("| ------ | -------------------: | --------------------------: |");
        for (name, f) in &engines {
            let mut counts = Vec::new();
            let mut recalls = Vec::new();
            for q in &queries {
                let got = f(&q.text);
                counts.push(got.len() as f64);
                let truth = ground_truth_dl1(&ds, &q.text);
                recalls.push(recall(&got, &truth));
            }
            println!(
                "| {name} | {:.1} | {} |",
                mean(&counts),
                pct(mean(&recalls))
            );
        }
        // wordtree's dedicated spell-check surface, configured exhaustive via
        // `Caps::uniform`: the bounded default (the `wordtree` row above, the
        // merged `suggestions()`) favours completions for valid-word queries, but
        // `corrections_with` opens the cap to match symspell's complete DL≤1 set.
        let mut ex_counts = Vec::new();
        let mut ex_recalls = Vec::new();
        for q in &queries {
            let got: Vec<String> = tree
                .corrections_with(&q.text, |_| true, Caps::uniform(64))
                .into_iter()
                .filter_map(|s| ds.word_of_expr(s.expr_index).map(|w| w.to_string()))
                .collect();
            ex_counts.push(got.len() as f64);
            let truth = ground_truth_dl1(&ds, &q.text);
            ex_recalls.push(recall(&got, &truth));
        }
        println!(
            "| wordtree (corrections, exhaustive) | {:.1} | {} |",
            mean(&ex_counts),
            pct(mean(&ex_recalls))
        );
        println!();

        // --- 3. Autocomplete top-5 agreement ---------------------------------
        println!(
            "### Autocomplete — top-5 agreement with the frequency oracle ({PREFIXES} prefixes)\n"
        );
        let prefixes = prefix_query_set(&ds, PREFIXES);
        // The combined suggestions() spends part of its 6-result budget on fuzzy
        // spellings, so its completions are diluted; completions() gives the whole
        // budget to extensions — the fair autocomplete path. Show both.
        let wt_sugg_recall: Vec<f64> = prefixes
            .iter()
            .map(|p| {
                let got: Vec<String> = wordtree_suggest_words(&tree, &ds, p)
                    .into_iter()
                    .take(5)
                    .collect();
                recall(&got, &ground_truth_prefix(&ds, p, 5))
            })
            .collect();
        let wt_comp_recall: Vec<f64> = prefixes
            .iter()
            .map(|p| {
                let got: Vec<String> = wordtree_complete_words(&tree, &ds, p)
                    .into_iter()
                    .take(5)
                    .collect();
                recall(&got, &ground_truth_prefix(&ds, p, 5))
            })
            .collect();
        let trie = build_pruning_trie(&ds.rows);
        let pt_recall: Vec<f64> = prefixes
            .iter()
            .map(|p| {
                let got = pruning_prefix_words(&trie, p, 5);
                recall(&got, &ground_truth_prefix(&ds, p, 5))
            })
            .collect();
        println!("| engine | recall@5 |");
        println!("| ------ | -------: |");
        println!(
            "| wordtree (suggestions) | {} |",
            pct(mean(&wt_sugg_recall))
        );
        println!(
            "| wordtree (completions) | {} |",
            pct(mean(&wt_comp_recall))
        );
        println!("| pruning-trie | {} |", pct(mean(&pt_recall)));
        println!();
    }
}
