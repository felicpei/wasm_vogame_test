#![feature(fundamental)]

pub mod userdata_dir;

pub use userdata_dir::userdata_dir;

/// Allows downstream crates to conditionally do things based on whether tracy
/// is enabled without having to expose a cargo feature themselves.

#[macro_export]
macro_rules! plot {
    ($name:expr, $value:expr) => {
        // type check
        let _: f64 = $value;
    };
}

// Panic in debug or tests, warn in release
#[macro_export]
macro_rules! dev_panic {
    ($msg:expr) => {
        if cfg!(any(debug_assertions, test)) {
            panic!("{}", $msg);
        } else {
            tracing::error!("{}", $msg);
        }
    };

    ($msg:expr, or return $result:expr) => {
        if cfg!(any(debug_assertions, test)) {
            panic!("{}", $msg);
        } else {
            tracing::warn!("{}", $msg);
            return $result;
        }
    };
}

// https://discordapp.com/channels/676678179678715904/676685797524766720/723358438943621151
#[macro_export]
macro_rules! span {
    ($guard_name:tt, $level:ident, $name:expr, $($fields:tt)*) => {
        let span = tracing::span!(tracing::Level::$level, $name, $($fields)*);
        let $guard_name = span.enter();
    };
    ($guard_name:tt, $level:ident, $name:expr) => {
        let span = tracing::span!(tracing::Level::$level, $name);
        let $guard_name = span.enter();
    };
    ($guard_name:tt, $name:expr) => {
        let span = tracing::span!(tracing::Level::TRACE, $name);
        let $guard_name = span.enter();
    };
    ($guard_name:tt, $no_tracy_name:expr, $tracy_name:expr) => {
        $crate::span!($guard_name, $no_tracy_name);
    };
}

pub struct ProfSpan;

/// Like the span macro but only used when profiling and not in regular tracing
/// operations
#[macro_export]
macro_rules! prof_span {
    ($guard_name:tt, $name:expr) => {
        let $guard_name = $crate::ProfSpan;
    };
    // Shorthand for when you want the guard to just be dropped at the end of the scope instead
    // of controlling it manually
    ($name:expr) => {};
}

/// There's no guard, but really this is actually the guard
pub struct GuardlessSpan {
    span: tracing::Span,
    subscriber: tracing::Dispatch,
}

impl GuardlessSpan {
    pub fn new(span: tracing::Span) -> Self {
        let subscriber = tracing::dispatcher::get_default(|d| d.clone());
        if let Some(id) = span.id() {
            subscriber.enter(&id)
        }
        Self { span, subscriber }
    }
}

impl Drop for GuardlessSpan {
    fn drop(&mut self) {
        if let Some(id) = self.span.id() {
            self.subscriber.exit(&id)
        }
    }
}

#[macro_export]
macro_rules! no_guard_span {
    ($level:ident, $name:expr, $($fields:tt)*) => {
        GuardlessSpan::new(
            tracing::span!(tracing::Level::$level, $name, $($fields)*)
        )
    };
    ($level:ident, $name:expr) => {
        GuardlessSpan::new(
            tracing::span!(tracing::Level::$level, $name)
        )
    };
    ($name:expr) => {
        GuardlessSpan::new(
            tracing::span!(tracing::Level::TRACE, $name)
        )
    };
}
