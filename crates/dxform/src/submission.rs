use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture};

use crate::FormError;

#[derive(Clone)]
pub struct SubmitContext<T> {
    pub values: T,
    pub submit_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmitError {
    pub message: Option<String>,
    pub errors: Vec<FormError>,
}

impl SubmitError {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: Some(message.into()),
            errors: Vec::new(),
        }
    }

    pub fn errors(errors: Vec<FormError>) -> Self {
        Self {
            message: None,
            errors,
        }
    }
}

type SubmitCallback<T> =
    Rc<dyn Fn(SubmitContext<T>) -> LocalBoxFuture<'static, std::result::Result<(), SubmitError>>>;

pub struct SubmitHandler<T>(SubmitCallback<T>);

impl<T> Clone for SubmitHandler<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> SubmitHandler<T> {
    pub fn sync(
        f: impl Fn(SubmitContext<T>) -> std::result::Result<(), SubmitError> + 'static,
    ) -> Self
    where
        T: 'static,
    {
        Self(Rc::new(move |ctx| {
            futures::future::ready(f(ctx)).boxed_local()
        }))
    }

    pub fn async_(
        f: impl Fn(SubmitContext<T>) -> LocalBoxFuture<'static, std::result::Result<(), SubmitError>>
        + 'static,
    ) -> Self {
        Self(Rc::new(f))
    }

    pub(crate) async fn submit(
        &self,
        ctx: SubmitContext<T>,
    ) -> std::result::Result<(), SubmitError> {
        (self.0)(ctx).await
    }
}
