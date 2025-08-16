//! Handler traits and common patterns
//!
//! This module defines common traits and patterns for CLI command handlers
//! to reduce code duplication and provide consistent interfaces.

use super::super::CliContext;

/// Trait for handlers that can be created from a CLI context
pub trait HandlerFactory<'a> {
    type Handler;
    
    /// Create a new handler instance from the given context
    fn create(context: &'a CliContext) -> Self::Handler;
}

/// Trait for stateless handlers that don't require context
pub trait StatelessHandlerFactory {
    type Handler;
    
    /// Create a new stateless handler instance
    fn create() -> Self::Handler;
}

/// Macro to reduce boilerplate for handlers that follow the standard pattern
macro_rules! impl_context_handler {
    ($handler:ty) => {
        impl<'a> crate::cli::handlers::traits::HandlerFactory<'a> for $handler {
            type Handler = Self;
            
            fn create(context: &'a crate::cli::CliContext) -> Self::Handler {
                Self::new(context)
            }
        }
    };
}

/// Macro to reduce boilerplate for stateless handlers
macro_rules! impl_stateless_handler {
    ($handler:ty) => {
        impl crate::cli::handlers::traits::StatelessHandlerFactory for $handler {
            type Handler = Self;
            
            fn create() -> Self::Handler {
                Self::new()
            }
        }
    };
}

// Re-export macros for use in handler modules
pub(crate) use impl_context_handler;
pub(crate) use impl_stateless_handler;


/// Factory struct for creating handlers with reduced boilerplate
pub struct HandlerBuilder<'a> {
    context: &'a CliContext,
}

impl<'a> HandlerBuilder<'a> {
    /// Create a new handler builder
    pub fn new(context: &'a CliContext) -> Self {
        Self { context }
    }
    
    /// Create a handler that requires context
    pub fn create_with_context<F>(&self) -> F::Handler
    where
        F: HandlerFactory<'a>,
    {
        F::create(self.context)
    }
    
    /// Create a stateless handler
    pub fn create_stateless<F>() -> F::Handler
    where
        F: StatelessHandlerFactory,
    {
        F::create()
    }
}