use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    BearerEnv(&'static str),
    BearerStatic(Arc<str>),
}
