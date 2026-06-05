use super::{Node, NodeRef};
use crate::{Builder, trie::TreeFn};
use insta::*;

/// Returns (first_child_pos, node_char, percentile, is_folder, max_child_percentile, is_last_sibling, expr_index)
fn read(tree: &crate::Tree, pos: usize) -> (u32, char, u16, bool, u16, bool, Option<u32>) {
    let cursor = NodeRef::new(
        &tree.vec,
        &tree.word_bits,
        &tree.rank_index,
        &tree.values,
        pos,
    );
    (
        cursor.first_child_node_pos(),
        cursor.char(),
        cursor.percentile(),
        cursor.is_folder(),
        cursor.max_child_percentile(),
        cursor.is_last_sibling(),
        cursor.expr_index(),
    )
}

#[test]
fn test_serialize() {
    let mut generator = Builder::new();
    generator.add_words(vec![
        ("aa", 21, 1),
        ("ab1", 22, 2),
        ("ab2", 23, 3),
        ("add", 24, 4),
        ("aee", 25, 5),
        ("aff", 26, 6),
        ("aff1", 27, 7),
        ("aff2", 28, 8),
        ("aff3", 29, 9),
        ("aft", 30, 10),
        ("agg", 31, 11),
    ]);

    generator.organize_into_folders(3);

    let tree = generator.to_tree();
    // (first_child_pos, node_char, percentile, is_folder, max_child_percentile, is_last_sibling, expr_index)
    assert_eq!(read(&tree, 0), (1, 'a', 0, true, 31, true, None)); // a:
    assert_eq!(read(&tree, 1), (0, 'a', 21, false, 0, false, Some(1))); // aa
    assert_eq!(read(&tree, 2), (7, 'b', 0, true, 23, false, None)); // ab:
    assert_eq!(read(&tree, 3), (9, 'd', 0, false, 24, false, None)); // ad
    assert_eq!(read(&tree, 4), (10, 'e', 0, false, 25, false, None)); // ae
    assert_eq!(read(&tree, 5), (11, 'f', 0, true, 30, false, None)); // af
    assert_eq!(read(&tree, 6), (13, 'g', 0, false, 31, true, None)); // ag
    assert_eq!(read(&tree, 7), (0, '1', 22, false, 0, false, Some(2))); // ab1
    assert_eq!(read(&tree, 8), (0, '2', 23, false, 0, true, Some(3))); // ab2
    assert_eq!(read(&tree, 9), (0, 'd', 24, false, 0, true, Some(4))); // add
    assert_eq!(read(&tree, 10), (0, 'e', 25, false, 0, true, Some(5))); // aee
    assert_eq!(read(&tree, 11), (14, 'f', 26, true, 29, false, Some(6))); // aff
    assert_eq!(read(&tree, 12), (0, 't', 30, false, 0, true, Some(10))); // aft
    assert_eq!(read(&tree, 13), (0, 'g', 31, false, 0, true, Some(11))); // agg
    assert_eq!(read(&tree, 14), (0, '1', 27, false, 0, false, Some(7))); // aff1
    assert_eq!(read(&tree, 15), (0, '2', 28, false, 0, false, Some(8))); // aff2
    assert_eq!(read(&tree, 16), (0, '3', 29, false, 0, true, Some(9))); // aff3

    assert_snapshot!(tree);

    assert_eq!(tree.vec.len(), 17 * Node::BYTES_PER_NODE);
}

#[test]
fn test_iter() {
    let mut generator = Builder::new();
    generator.add_words(vec![
        ("aa", 21, 1),
        ("ab1", 22, 2),
        ("ab2", 23, 3),
        ("add", 24, 4),
        ("aee", 25, 5),
        ("aff", 26, 6),
        ("aff1", 27, 7),
        ("aff2", 28, 8),
        ("aff3", 29, 9),
        ("aft", 30, 10),
        ("agg", 31, 11),
    ]);

    generator.organize_into_folders(3);

    let nodes = generator.to_tree();
    // see snap for test_serialize
    let mut cursor = nodes.root();
    assert_eq!(cursor.pos(), 0);
    assert!(!cursor.move_to_next_sibling());
    assert_eq!(cursor.pos(), 0);
    assert_eq!(cursor.char(), 'a');
    assert!(cursor.move_to_sibling_matching('a'));
    assert_eq!(cursor.pos(), 0);
    assert!(!cursor.move_to_sibling_matching('c'));
    assert_eq!(cursor.pos(), 0);
    assert_eq!(cursor.children().map(|c| c.pos()), Some(1));
    assert!(cursor.move_to_first_child());
    assert_eq!(cursor.pos(), 1);
    assert!(cursor.move_to_sibling_matching('e')); // ae
    assert_eq!(cursor.char(), 'e');
}

#[test]
fn test_path() {
    let mut generator = Builder::new();
    let words = vec![
        ("ad1", 2, 1),
        ("ad2", 2, 2),
        ("ad3", 2, 3),
        ("add", 2, 4),
        ("adder", 2, 5),
        ("adder1", 2, 6),
        ("adder2", 2, 7),
        ("addition", 2, 8),
    ];
    generator.add_words(words.clone());
    generator.organize_into_folders(3);
    let ser = generator.to_tree();
    // assert_snapshot!(generator.root().borrow());
    assert_eq!(ser.path_of("a"), Some("/".to_string()));
    assert_eq!(ser.path_of("ad"), Some("a/".to_string()));
    assert_eq!(ser.path_of("ad1"), Some("ad/".to_string()));
    assert_eq!(ser.path_of("ad2"), Some("ad/".to_string()));
    assert_eq!(ser.path_of("ad3"), Some("ad/".to_string()));
    assert_eq!(ser.path_of("add"), Some("ad/".to_string()));
    assert_eq!(ser.path_of("adder"), Some("adde/".to_string()));
    assert_eq!(ser.path_of("adder1"), Some("adde/".to_string()));
    assert_eq!(ser.path_of("adder2"), Some("adde/".to_string()));
    assert_eq!(ser.path_of("addi"), Some("add/".to_string()));
    assert_eq!(ser.path_of("addit"), Some("add/".to_string()));
    assert_eq!(ser.path_of("addition"), Some("add/".to_string()));
}

#[test]
fn test_folders_and_words() {
    let mut generator = Builder::new();
    generator.add_words(vec![
        ("add", 2, 1),
        ("adder", 2, 2),
        ("aardvark", 2, 3),
        ("apple", 2, 4),
        ("addition", 2, 5),
        ("ape", 2, 6),
        ("app", 2, 7),
    ]);

    generator.organize_into_folders(3);
    let ser = generator.to_tree();
    // assert_snapshot!(generator.root().borrow());

    assert_eq!(
        ser.root().content_of("a"),
        vec![
            "aardvark.".to_string(),
            "ad/".to_string(),
            "ap/".to_string()
        ]
    );
    assert_eq!(
        ser.root().content_of("ad"),
        vec![
            "add.".to_string(),
            "addition.".to_string(),
            "adder.".to_string(),
        ]
    );
    assert_eq!(
        ser.root().content_of("ap"),
        vec!["ape.".to_string(), "app.".to_string(), "apple.".to_string(),]
    );
    assert_eq!(ser.root().content_of(""), vec!["a/".to_string(),]);
}
