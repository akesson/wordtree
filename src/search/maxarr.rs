use std::fmt::Display;

pub trait MaxVal<V: Copy + Ord + Display> {
    fn value(&self) -> V;
}

/// A vector behaving like an array in that it has a fixed
/// capacity and will only keep items with the greatest value.
pub struct MaxArr<V, T>
where
    V: Copy + Ord + Display,
    T: Display + MaxVal<V>,
{
    vec: Vec<T>,
    min_value: Option<V>,
    capacity: usize,
}

impl<V, T> MaxArr<V, T>
where
    V: Copy + Ord + Display,
    T: Display + MaxVal<V>,
{
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vec: Vec::with_capacity(capacity),
            min_value: None,
            capacity,
        }
    }

    pub fn add(&mut self, item: T) -> bool {
        let val = item.value();
        if Some(val) < self.min_value {
            return false;
        }
        if !self.full() {
            self.vec.push(item);
        } else if let Some(idx) = self.min_idx() {
            self.vec[idx] = item;
        } else if self.capacity > 0 {
            panic!(
                "new: {} <= min: {} for {}",
                item.value(),
                self.min_value
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or("-".to_string()),
                item
            );
        }
        self.min_value = self.calculate_min_freq();
        true
    }

    pub fn min_value(&self) -> Option<V> {
        self.min_value
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ T> {
        self.vec.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.vec.into_iter()
    }

    // the required min_freq (if there's empty slots then -1)
    fn calculate_min_freq(&self) -> Option<V> {
        if !self.full() {
            return None;
        }
        self.vec.iter().map(|e| e.value()).min()
    }

    #[inline]
    fn full(&self) -> bool {
        self.capacity <= self.vec.len()
    }

    /// the index of the entry with the lowest freq_score
    fn min_idx(&self) -> Option<usize> {
        self.vec
            .iter()
            .map(|v| v.value())
            .enumerate()
            .min_by(|(_, a), (_, b)| a.cmp(b))
            .map(|(i, _)| i)
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }
}

impl<V, T> std::fmt::Display for MaxArr<V, T>
where
    V: Copy + Ord + Display,
    T: Display + MaxVal<V>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.vec
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

#[cfg(test)]
#[derive(Clone, derive_more::Constructor)]
struct IdxAndPercent {
    percent: u16,
    idx: u32,
}

#[cfg(test)]
impl MaxVal<u16> for IdxAndPercent {
    fn value(&self) -> u16 {
        self.percent
    }
}

#[cfg(test)]
impl std::fmt::Display for IdxAndPercent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} {:.1}%", self.idx, self.percent as f32 / 10.0)
    }
}

#[test]
fn test_propose() {
    let mut sel = MaxArr::<u16, IdxAndPercent>::with_capacity(2);

    assert_eq!("", sel.to_string());
    sel.add(IdxAndPercent::new(232, 1));
    assert_eq!("1 23.2%", sel.to_string());

    sel.add(IdxAndPercent::new(212, 2));
    assert_eq!("1 23.2%, 2 21.2%", sel.to_string());

    // propose smaller freq (should be discarded)
    sel.add(IdxAndPercent::new(202, 3));
    assert_eq!("1 23.2%, 2 21.2%", sel.to_string());

    sel.add(IdxAndPercent::new(302, 4));
    assert_eq!("1 23.2%, 4 30.2%", sel.to_string());

    sel.add(IdxAndPercent::new(352, 5));
    assert_eq!("5 35.2%, 4 30.2%", sel.to_string());
}
