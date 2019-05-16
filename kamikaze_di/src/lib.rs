#![feature(specialization)]

mod container;

pub use container::{Container, ContainerBuilder};
pub use container::auto_resolver::{Resolvable, AutoResolver};
pub use container::omni_resolver::OmniResolver;

pub type Result<T> = std::result::Result<T, String>;
