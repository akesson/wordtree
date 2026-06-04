#[macro_use]
extern crate ego_tree;

use ego_tree::Tree;

#[test]
fn new() {
    let tree = Tree::new('a');
    let root = tree.root();
    assert_eq!(&'a', root.value());
    assert_eq!(None, root.parent());
    assert_eq!(None, root.prev_sibling());
    assert_eq!(None, root.next_sibling());
    assert_eq!(None, root.first_child());
    assert_eq!(None, root.last_child());
}

#[test]
fn root() {
    let tree = Tree::new('a');
    assert_eq!(&'a', tree.root().value());
}

#[test]
fn root_mut() {
    let mut tree = Tree::new('a');
    assert_eq!(&'a', tree.root_mut().value());
}

#[test]
fn orphan() {
    let mut tree = Tree::new('a');
    let mut orphan = tree.orphan('b');
    assert_eq!(&'b', orphan.value());
    assert!(orphan.parent().is_none());
}

#[test]
fn get() {
    let tree = Tree::new('a');
    let id = tree.root().id();
    assert_eq!(Some(tree.root()), tree.get(id));
}

#[test]
fn get_mut() {
    let mut tree = Tree::new('a');
    let id = tree.root().id();
    assert_eq!(Some('a'), tree.get_mut(id).map(|mut n| *n.value()));
}

#[test]
fn clone() {
    let one = Tree::new('a');
    let two = one.clone();
    assert_eq!(one, two);
}

#[test]
fn eq() {
    let one = Tree::new('a');
    let two = Tree::new('a');
    assert_eq!(one, two);
}

#[test]
#[should_panic]
fn neq() {
    let one = Tree::new('a');
    let two = Tree::new('b');
    assert_eq!(one, two);
}

#[test]
fn search() {
    let mut tree = tree! {
        1 => {
            2 => {
                2, 3, 1
            },
            3 => {
                2, 3, 1
            },
            1 => {
                2, 3, 1
            }
        }
    };
    tree.sort_by_key(|v| *v);
    assert_eq!(
        format!("{:#?}", tree),
        "Tree { 1 => { 1 => { 1, 2, 3 }, 2 => { 1, 2, 3 }, 3 => { 1, 2, 3 } } }"
    );

    tree.sort_by_key(|v| std::cmp::Reverse(*v));
    assert_eq!(
        format!("{:#?}", tree),
        "Tree { 1 => { 3 => { 3, 2, 1 }, 2 => { 3, 2, 1 }, 1 => { 3, 2, 1 } } }"
    );
}

#[derive(Default)]
struct NumString(String);

impl std::ops::AddAssign<NumString> for NumString {
    #[inline]
    fn add_assign(&mut self, other: NumString) {
        self.0.push_str(&other.0)
    }
}

impl NumString {
    fn add(&mut self, num: usize) {
        self.0.push_str(&format!(" {}", num))
    }
}

#[test]
fn depth_first_fold_order() {
    let mut tree = tree! {
        13 => {
            4 => {
                1, 2, 3
            },
            8 => {
                5, 6, 7
            },
            12 => {
                9, 10, 11
            }
        }
    };
    let out = tree.depth_first_fold(|v, acc: &mut NumString| acc.add(*v));
    assert_eq!(out.0, " 1 2 3 4 5 6 7 8 9 10 11 12 13");
}

#[test]
fn depth_first_fold_acc() {
    let mut tree: Tree<usize> = tree! {
        1 => {
            1 => {
                1, 1, 1
            },
            1 => {
                1, 1, 1
            },
            1 => {
                1, 1, 1
            }
        }
    };
    let out = tree.depth_first_fold(|v, acc: &mut usize| *acc += *v);
    assert_eq!(out, 13);
}
