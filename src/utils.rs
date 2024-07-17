use crate::imports::*;

pub trait GroupExtension<K1, K2, V, I>
where
    K1: std::hash::Hash + Ord,
    K2: std::hash::Hash + Ord,
    I: IntoIterator<Item = (K1, K2, V)>,
{
    // fn group_from(v: I) -> AHashMap<K, Vec<V>>;
    fn group_from(v: I) -> AHashMap<K1, AHashMap<K2, V>>;
    // fn group_map_from(v: I) -> AHashMap<K, Vec<V>>;
}

impl<K1, K2, V, I> GroupExtension<K1, K2, V, I> for AHashMap<K1, AHashMap<K2, V>>
where
    K1: std::hash::Hash + Ord,
    K2: std::hash::Hash + Ord,
    I: IntoIterator<Item = (K1, K2, V)>,
{
    fn group_from(v: I) -> AHashMap<K1, AHashMap<K2, V>> {
        let mut result = AHashMap::<K1, AHashMap<K2, V>>::new();
        for (k1, k2, v) in v {
            result.entry(k1).or_default().insert(k2, v);
        }
        result
    }
}
