pub mod gemini;

use std::{future::Future, path::Path, pin::Pin};

use crate::error::BridgeError;

pub type LlmFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BridgeError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Voice,
    Video,
    ShortVideo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmMediaRequest<'a> {
    pub kind: MediaKind,
    pub path: &'a Path,
    pub mime_type: &'a str,
    pub system_prompt: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmOutput {
    pub provider: String,
    pub model: String,
    pub text: String,
}

pub trait LlmProvider: Send + Sync {
    fn process_media<'a>(&'a self, request: LlmMediaRequest<'a>) -> LlmFuture<'a, LlmOutput>;
}
