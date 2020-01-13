pub trait ExistsExtension<T> {
    fn exists<P: FnOnce(&T) -> bool>(&self, predicate: P) -> bool;
}

impl <T> ExistsExtension<T> for Option<T> {
    fn exists<P: FnOnce(&T) -> bool>(&self, predicate: P) -> bool {
        match self {
            Some(value) => predicate(value),
            None => false,
        }
    }
}
