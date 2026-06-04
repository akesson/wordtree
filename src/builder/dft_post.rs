use std::collections::VecDeque;
use std::iter::FusedIterator;

/// [Depth-First Traversal] in Post-Order.
///
/// [Depth-First Traversal]: https://en.wikipedia.org/wiki/Tree_traversal
///
/// # Cycles
///
/// `DftPost` does not handle cycles. If any
/// cycles are present, then `DftPost` will
/// result in an infinite (never ending)
/// [`Iterator`].
///
/// [`Iterator`]: https://doc.rust-lang.org/stable/std/iter/trait.Iterator.html
///
#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct DftPost<T, F, I>
where
    T: Sized + Clone,
    F: FnMut(&T) -> I,
    I: Iterator<Item = T>,
{
    /// `iter_children(self.queue[i])` has been called for all `i < visited`
    visited: usize,
    current_depth: usize,
    queue: VecDeque<(usize, T)>,
    iter_children: F,
}

impl<T, F, I> DftPost<T, F, I>
where
    T: Sized + Clone,
    F: FnMut(&T) -> I,
    I: Iterator<Item = T>,
{
    /// Creates a `DftPost`, where `root` is the
    /// starting `Node`.
    ///
    /// The `iter_children` [`FnMut`] is (lazily) called
    /// for each `Node` as needed, where the
    /// returned [`Iterator`] produces the child
    /// `Node`s for the given `Node`.
    ///
    /// [`Iterator`]: https://doc.rust-lang.org/stable/std/iter/trait.Iterator.html
    ///
    /// *[See `DftPost` for more information.][`DftPost`]*
    ///
    /// [`DftPost`]: struct.DftPost.html
    ///
    /// # "`FnOnce`"
    ///
    /// The [`FnMut`] is a [`FnOnce`] from the point-of-view of
    /// a `Node`, as `iter_children` is at most called once for
    /// each individual `Node`.
    ///
    /// [`FnMut`]: https://doc.rust-lang.org/std/ops/trait.FnMut.html
    /// [`FnOnce`]: https://doc.rust-lang.org/std/ops/trait.FnOnce.html
    ///
    /// # `FusedIterator`
    ///
    /// While `DftPost` does not require [`FusedIterator`],
    /// it assumes that no `Node`s are produced after
    /// a `None`.
    ///
    /// [`FusedIterator`]: https://doc.rust-lang.org/stable/std/iter/trait.FusedIterator.html
    #[inline]
    pub fn new(root: T, iter_children: F) -> Self {
        Self {
            visited: 0,
            current_depth: 0,
            queue: VecDeque::from(vec![(0, root)]),
            iter_children,
        }
    }
}

impl<T, F, I> Iterator for DftPost<T, F, I>
where
    T: Sized + Clone,
    F: FnMut(&T) -> I,
    I: Iterator<Item = T>,
{
    type Item = (usize, T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.queue.is_empty() {
            None
        } else {
            // Exhaust all children for the first branch
            loop {
                let i = self.visited;
                let (depth, node) = if i < self.queue.len() {
                    self.queue[i].clone()
                } else {
                    break;
                };

                // This node is not a child nor a sibling
                if self.current_depth > depth {
                    break;
                }

                let before_len = self.queue.len();

                for (j, child) in (self.iter_children)(&node).enumerate() {
                    self.queue.insert(i + j + 1, (depth + 1, child));
                }
                self.visited += 1;

                self.current_depth = depth;

                // The previous node produced no children
                if self.queue.len() == before_len {
                    break;
                }
            }

            let node = self.queue.remove(self.visited - 1).unwrap();
            self.visited -= 1;

            self.current_depth = node.0;

            Some(node)
        }
    }
}

impl<T, F, I> FusedIterator for DftPost<T, F, I>
where
    T: Sized + Clone,
    F: FnMut(&T) -> I,
    I: Iterator<Item = T>,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Node(&'static str, &'static [Node]);

    #[test]
    fn dft_post() {
        #[rustfmt::skip]
        let tree = Node("I", &[
            Node("D", &[
                Node("A", &[]),
                Node("C", &[
                    Node("B", &[])
                ])]),
            Node("H", &[
                Node("F", &[
                    Node("E", &[])
                ]),
                Node("G", &[])]),
        ]);

        let iter = DftPost::new(&tree, |node| node.1.iter());
        let mut iter = iter.map(|(depth, node)| (depth, node.0));

        assert_eq!(iter.next(), Some((2, "A")));
        assert_eq!(iter.next(), Some((3, "B")));
        assert_eq!(iter.next(), Some((2, "C")));
        assert_eq!(iter.next(), Some((1, "D")));
        assert_eq!(iter.next(), Some((3, "E")));
        assert_eq!(iter.next(), Some((2, "F")));
        assert_eq!(iter.next(), Some((2, "G")));
        assert_eq!(iter.next(), Some((1, "H")));
        assert_eq!(iter.next(), Some((0, "I")));
        assert_eq!(iter.next(), None);
    }
}
