#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamModel<T> {
    Stream(T),
    Compact { attempt: usize },
    Finish { reason: String },
}
