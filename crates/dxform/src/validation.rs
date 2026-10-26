use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture};

use crate::{FieldPath, FormError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationPhase {
    Mount,
    Change,
    Blur,
    Submit,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationStrategy {
    pub on_mount: bool,
    pub on_change: bool,
    pub on_blur: bool,
    pub on_submit: bool,
}

impl ValidationStrategy {
    pub fn never() -> Self {
        Self {
            on_mount: false,
            on_change: false,
            on_blur: false,
            on_submit: false,
        }
    }

    pub fn submit() -> Self {
        Self {
            on_submit: true,
            ..Self::never()
        }
    }

    pub fn change() -> Self {
        Self {
            on_change: true,
            on_submit: true,
            ..Self::never()
        }
    }

    pub fn blur() -> Self {
        Self {
            on_blur: true,
            on_submit: true,
            ..Self::never()
        }
    }

    pub fn should_validate(self, phase: ValidationPhase) -> bool {
        match phase {
            ValidationPhase::Mount => self.on_mount,
            ValidationPhase::Change => self.on_change,
            ValidationPhase::Blur => self.on_blur,
            ValidationPhase::Submit => self.on_submit,
            ValidationPhase::Manual => true,
        }
    }
}

impl Default for ValidationStrategy {
    fn default() -> Self {
        Self::submit()
    }
}

#[derive(Clone)]
pub struct FieldValidationContext<Parent, Value> {
    pub phase: ValidationPhase,
    pub path: FieldPath,
    pub value: Value,
    pub parent: Parent,
}

#[derive(Clone)]
pub struct ScopeValidationContext<T> {
    pub phase: ValidationPhase,
    pub path: FieldPath,
    pub value: T,
}

#[derive(Clone)]
pub struct ListValidationContext<Item> {
    pub phase: ValidationPhase,
    pub path: FieldPath,
    pub items: Vec<Item>,
}

#[derive(Clone)]
pub struct FormValidationContext<T> {
    pub phase: ValidationPhase,
    pub values: T,
}

type FieldValidationCallback<Parent, Value> =
    Rc<dyn Fn(FieldValidationContext<Parent, Value>) -> LocalBoxFuture<'static, Vec<FormError>>>;

pub struct FieldValidator<Parent, Value>(FieldValidationCallback<Parent, Value>);

impl<Parent, Value> Clone for FieldValidator<Parent, Value> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Parent, Value> FieldValidator<Parent, Value> {
    pub fn sync(
        f: impl Fn(FieldValidationContext<Parent, Value>) -> Vec<FormError> + 'static,
    ) -> Self
    where
        Parent: 'static,
        Value: 'static,
    {
        Self(Rc::new(move |ctx| {
            futures::future::ready(f(ctx)).boxed_local()
        }))
    }

    pub fn async_(
        f: impl Fn(FieldValidationContext<Parent, Value>) -> LocalBoxFuture<'static, Vec<FormError>>
        + 'static,
    ) -> Self {
        Self(Rc::new(f))
    }

    pub(crate) async fn validate(
        &self,
        ctx: FieldValidationContext<Parent, Value>,
    ) -> Vec<FormError> {
        (self.0)(ctx).await
    }
}

pub struct ScopeValidator<T>(
    Rc<dyn Fn(ScopeValidationContext<T>) -> LocalBoxFuture<'static, Vec<FormError>>>,
);

impl<T> Clone for ScopeValidator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> ScopeValidator<T> {
    pub fn sync(f: impl Fn(ScopeValidationContext<T>) -> Vec<FormError> + 'static) -> Self
    where
        T: 'static,
    {
        Self(Rc::new(move |ctx| {
            futures::future::ready(f(ctx)).boxed_local()
        }))
    }

    pub fn async_(
        f: impl Fn(ScopeValidationContext<T>) -> LocalBoxFuture<'static, Vec<FormError>> + 'static,
    ) -> Self {
        Self(Rc::new(f))
    }

    pub(crate) async fn validate(&self, ctx: ScopeValidationContext<T>) -> Vec<FormError> {
        (self.0)(ctx).await
    }
}

pub struct ListValidator<Item>(
    Rc<dyn Fn(ListValidationContext<Item>) -> LocalBoxFuture<'static, Vec<FormError>>>,
);

impl<Item> Clone for ListValidator<Item> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Item> ListValidator<Item> {
    pub fn sync(f: impl Fn(ListValidationContext<Item>) -> Vec<FormError> + 'static) -> Self
    where
        Item: 'static,
    {
        Self(Rc::new(move |ctx| {
            futures::future::ready(f(ctx)).boxed_local()
        }))
    }

    pub fn async_(
        f: impl Fn(ListValidationContext<Item>) -> LocalBoxFuture<'static, Vec<FormError>> + 'static,
    ) -> Self {
        Self(Rc::new(f))
    }

    pub(crate) async fn validate(&self, ctx: ListValidationContext<Item>) -> Vec<FormError> {
        (self.0)(ctx).await
    }
}

pub struct FormValidator<T>(
    Rc<dyn Fn(FormValidationContext<T>) -> LocalBoxFuture<'static, Vec<FormError>>>,
);

impl<T> Clone for FormValidator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> FormValidator<T> {
    pub fn sync(f: impl Fn(FormValidationContext<T>) -> Vec<FormError> + 'static) -> Self
    where
        T: 'static,
    {
        Self(Rc::new(move |ctx| {
            futures::future::ready(f(ctx)).boxed_local()
        }))
    }

    pub fn async_(
        f: impl Fn(FormValidationContext<T>) -> LocalBoxFuture<'static, Vec<FormError>> + 'static,
    ) -> Self {
        Self(Rc::new(f))
    }

    pub(crate) async fn validate(&self, ctx: FormValidationContext<T>) -> Vec<FormError> {
        (self.0)(ctx).await
    }
}
