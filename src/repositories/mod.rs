pub mod engine;
pub mod session;

use std::sync::Arc;

use axum::Extension;

use crate::repositories::{engine::EngineRepositoryImpl, session::InMemorySessionRepositoryImpl};

pub type EngineRepoExt = Extension<Arc<EngineRepositoryImpl>>;
pub type SessionRepoExt = Extension<Arc<InMemorySessionRepositoryImpl>>;
