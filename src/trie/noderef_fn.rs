use std::ops::Deref;

use super::{NodeRef, StringDelim};

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
    pub fn content_of_children(&mut self, prefix: &str) -> Vec<String> {
        let mut to_search: Vec<(NodeRef<'a, V>, String)> = Vec::new();
        let mut found: Vec<String> = Vec::new();

        loop {
            let chr = self.char();
            if self.is_folder() {
                found.push(String::prefix_two_char(prefix, chr, '/'));
                // found.push(String::two_char(chr, '/'));
            }
            if self.is_word() {
                found.push(String::prefix_two_char(prefix, chr, '.'));
            }
            if !self.is_folder()
                && let Some(children) = self.children()
            {
                to_search.push((children, String::prefix_one_char(prefix, chr)));
            }
            while let Some((mut childcursor, prefix)) = to_search.pop() {
                loop {
                    let chr = childcursor.char();
                    if childcursor.is_word() {
                        found.push(String::prefix_two_char(&prefix, chr, '.'));
                    }
                    if !childcursor.is_folder()
                        && let Some(children) = childcursor.children()
                    {
                        to_search.push((children, String::prefix_one_char(&prefix, chr)));
                    }
                    if !childcursor.move_to_next_sibling() {
                        break;
                    }
                }
            }
            if !self.move_to_next_sibling() {
                break;
            }
        }
        found
    }

    pub fn path_of_children(&mut self, word: &str) -> Option<String> {
        let mut path = String::with_capacity(word.len() * 2);
        let mut iter = word.chars().peekable();
        while let Some(chr) = iter.next() {
            if self.move_to_sibling_matching(chr) {
                if self.is_folder() {
                    path.push(chr);
                }
                if !self.move_to_first_child() && iter.peek().is_some() {
                    return None;
                }
            } else {
                return None;
            }
        }
        path.push('/');
        Some(path)
    }

    /// get the path of a word. Used for finding the filesystem path of a word
    /// (for reading wiktionary file)
    pub fn path_and_content_of(&mut self, prefix: &str) -> Option<(String, Vec<String>)> {
        let path = self.path_of_children(prefix)?;
        // here we know that the prefix was found and we have the path
        // now find the content
        let content = self.content_of_children(prefix);
        Some((path, content))
    }

    /// Find the path of the word. Ex: apple -> /a/p
    pub fn path_of(&mut self, word: &str) -> Option<String> {
        let mut word = String::from(word);
        word.pop();
        self.path_of_children(&word)
    }

    // get the folder and words for a prefix. Used when browsing via index.
    pub fn content_of(&mut self, prefix: &str) -> Vec<String> {
        let mut iter = prefix.chars().peekable();
        while let Some(chr) = iter.next() {
            if self.move_to_sibling_matching(chr) {
                if !self.move_to_first_child() && iter.peek().is_some() {
                    return Vec::new();
                }
            } else {
                return Vec::new();
            }
        }

        self.content_of_children(prefix)
    }

    pub fn index_of(&mut self, word: &str) -> Option<u32> {
        let mut iter = word.chars().peekable();
        while let Some(chr) = iter.next() {
            if self.move_to_sibling_matching(chr) {
                let is_last_searched_char = iter.peek().is_none();

                if is_last_searched_char {
                    return self.expr_index();
                }

                if !self.move_to_first_child() {
                    return None;
                }
            } else {
                return None;
            }
        }
        None
    }
}
