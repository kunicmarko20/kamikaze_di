use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

/// Dependencies have to be registered beforehand, how you do
/// that depends on the implementing type.
///
/// Dependencies can be shared across multiple use points. In
/// garbage collected languages, these dependencies would
/// naturally live on the heap and the garbage collector would
/// take care of deallocating them.
///
/// In rust, someone must own them. Naturally, this will be
/// the dependency injection container.
///
/// At first thought, returning references would be OK. However,
/// this may lead to problems when dealing with lifetimes, so we
/// just return Rc<T> instead.
///
/// If you need to resolve a trait, use `Box<Trait>`.
///
pub trait DependencyResolver<T: 'static> {
    /// Resolve a dependency
    ///
    /// # Examples
    ///
    /// ```
    /// use std::rc::Rc;
    /// use kamikaze_di::{Container, DependencyResolver};
    ///
    /// let mut container = Container::new();
    /// container.register::<u32>(42);
    ///
    /// let resolved: Rc<u32> = container.resolve().unwrap();
    /// assert_eq!(*resolved, 42);
    /// ```
    fn resolve(&self) -> ResolveResult<T>;
}

/// DependencyResolver implementor
///
/// You can register shared dependencies (they will act like singletons)
/// with the register() and register_builder() functions.
///
/// You can register factories for dependencies (each request for them
/// will produce a new instance) with the register_factory() and
/// register_automatic_factory() functions.
///
/// Register fuctions return an Err(String) when trying to register the same
/// dependency twice.
///
/// # Examples
///
/// ```
/// use std::rc::Rc;
/// use kamikaze_di::{Container, DependencyResolver};
///
/// let mut container = Container::new();
/// let result_1 = container.register::<u32>(42);
/// let result_2 = container.register::<u32>(43);
///
/// assert!(result_1.is_ok());
/// assert!(result_2.is_err());
/// ```
pub struct Container {
    resolvers: RefCell<HashMap<TypeId, Resolver>>,
}

impl<T: 'static> DependencyResolver<T> for Container {
    fn resolve(&self) -> ResolveResult<T> {
        self.get::<T>()
    }
}

// TODO these can be trait aliases, once that feature becomes stable
/// Factories can be called multiple times
pub type Factory<T> = FnMut(&Container) -> T;
/// Builders will only be called once
pub type Builder<T> = FnOnce(&Container) -> T;

impl Container {
    pub fn new() -> Container {
        Container {
            resolvers: RefCell::new(Default::default())
        }
    }

    /// Registeres a dependency directly
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::rc::Rc;
    /// # use kamikaze_di::{Container, DependencyResolver};
    ///
    /// let mut container = Container::new();
    /// let result = container.register::<u32>(42);
    ///
    /// assert!(result.is_ok());
    /// ```
    pub fn register<T: 'static>(&mut self, item: T) -> DiResult<()> {
        let resolver = Resolver::Shared(Rc::new(item));

        self.insert::<T>(resolver)
    }

    /// Registers a factory.
    ///
    /// Every call to get() will return a new dependency.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::rc::Rc;
    /// # use kamikaze_di::{Container, DependencyResolver};
    ///
    /// let mut container = Container::new();
    /// container.register::<i16>(43);
    ///
    /// let mut i = 0;
    /// container.register_factory::<i32, _>(move |container| {
    ///     i += 1;
    ///     let base: i16 = *container.resolve().unwrap();
    ///     let base: i32 = base.into();
    ///     base - i
    /// });
    ///
    /// let forty_two: Rc<i32> = container.resolve().unwrap();
    /// let forty_one: Rc<i32> = container.resolve().unwrap();
    ///
    /// assert_eq!(*forty_two, 42);
    /// assert_eq!(*forty_one, 41);
    /// ```
    pub fn register_factory<T, F>(&mut self, factory: F) -> DiResult<()>
        where F: (FnMut(&Container) -> T) + 'static,
              T: 'static
    {
        // we use double boxes so we can downcast to the inner box type
        // you can only downcast to Sized types, that's why we need an inner box
        // see call_factory() for use
        let boxed = Box::new(factory) as Box<(FnMut(&Container) -> T) + 'static>;
        let boxed = Box::new(boxed) as Box<Any>;
        let resolver = Resolver::Factory(RefCell::new(boxed));

        self.insert::<T>(resolver)
    }

    pub fn register_automatic_factory<T: 'static>(&mut self) -> DiResult<()> {
        //let resolver = Resolver::Factory(Box::new(|container| auto_factory::<T>(container)));

        //self.insert::<T>(resolver)
        unimplemented!("This will be implemented later")
    }

    /// Registers a builder.
    ///
    /// The dependency is created only when needed and after that
    /// it behaves as if registered via register(item).
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::rc::Rc;
    /// # use kamikaze_di::{Container, DependencyResolver};
    ///
    /// let mut container = Container::new();
    /// container.register::<i16>(43);
    ///
    /// container.register_builder::<i32, _>(|container| {
    ///     let base: i16 = *container.resolve().unwrap();
    ///     let base: i32 = base.into();
    ///     base - 1
    /// });
    ///
    /// container.register_builder::<i64, _>(|container| {
    ///     let base: i32 = *container.resolve().unwrap();
    ///     let base: i64 = base.into();
    ///     base - 1
    /// });
    /// let forty_one: Rc<i64> = container.resolve().unwrap();
    /// let forty_two: Rc<i32> = container.resolve().unwrap();
    ///
    /// assert_eq!(*forty_one, 41);
    /// assert_eq!(*forty_two, 42);
    /// ```
    pub fn register_builder<T, B>(&mut self, builder: B) -> DiResult<()>
        where B: (FnOnce(&Container) -> T) + 'static,
              T: 'static
    {
        let boxed = Box::new(builder) as Box<(FnOnce(&Container) -> T) + 'static>;
        let boxed = Box::new(boxed) as Box<Any>;
        let resolver = Resolver::Builder(boxed);

        self.insert::<T>(resolver)
    }

    /// Returns true if a dependency is registered
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::rc::Rc;
    /// # use kamikaze_di::{Container, DependencyResolver};
    ///
    /// let mut container = Container::new();
    /// container.register::<i16>(43);
    ///
    /// assert!(container.has::<i16>());
    /// assert!(!container.has::<i32>());
    /// ```
    pub fn has<T: 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();

        self.resolvers.borrow().contains_key(&type_id)
    }

    fn get<T: 'static>(&self) -> ResolveResult<T> {
        let item = self.resolve_as_any::<T>()?;

        Self::downcast::<T>(item)
    }

    fn downcast<T: 'static>(item: Rc<Any>) -> ResolveResult<T> {
        let raw = Rc::into_raw(item);

        // this should be safe as long as registration is safe
        Ok(unsafe {
            Rc::<T>::from_raw(raw as *const T)
        })
    }

    fn resolve_as_any<T: 'static>(&self) -> IntermediateResult {
        let type_id = TypeId::of::<T>();

        let resolver_type = self.get_resolver_type(&type_id);

        match resolver_type {
            Some(ResolverType::Factory) => self.call_factory::<T>(&type_id),
            Some(ResolverType::Builder) => {
                self.consume_builder::<T>()?;
                self.get_shared(&type_id)
            },
            Some(ResolverType::Shared) => self.get_shared(&type_id),
            None => Err(format!("Type not registered: {:?}", type_id)),
        }
    }

    fn get_resolver_type(&self, type_id: &TypeId) -> Option<ResolverType> {
        self.resolvers.borrow()
            .get(type_id)
            .map(|r| r.into())
    }

    fn call_factory<T: 'static>(&self, type_id: &TypeId) -> IntermediateResult {
        if let Resolver::Factory(cell) = self.resolvers.borrow().get(&type_id).unwrap() {
            let mut boxed = cell.borrow_mut();
            let factory = boxed.downcast_mut::<Box<Factory<T>>>().unwrap();

            let item = factory(self);

            return Ok(Rc::new(item));
        }

        panic!("Type {:?} not registered as factory", type_id)
    }

    fn consume_builder<T: 'static>(&self) -> DiResult<()> {
        let type_id = TypeId::of::<T>();

        let builder = if let Resolver::Builder(boxed) = self.resolvers.borrow_mut().remove(&type_id).unwrap() {
            boxed.downcast::<Box<Builder<T>>>().unwrap()
        } else {
            panic!("Type {:?} not registered as builder", type_id)
        };

        let item = builder(self);
        let resolver = Resolver::Shared(Rc::new(item));

        return self.insert::<T>(resolver);
    }

    fn get_shared(&self, type_id: &TypeId) -> IntermediateResult {
        if let Resolver::Shared(item) = self.resolvers.borrow().get(&type_id).unwrap() {
            return Ok(item.clone());
        }

        panic!("Type {:?} not registered as shared dependency", type_id)
    }

    fn insert<T: 'static>(&self, resolver: Resolver) -> DiResult<()> {
        let type_id = TypeId::of::<T>();

        if self.has::<T>() {
            return Err(format!("Container already has {:?}", type_id));
        }

        self.resolvers.borrow_mut().insert(type_id, resolver);

        Ok(())
    }
}

fn auto_factory<T>(container: &Container) -> Box<T> {
    unimplemented!()
}

enum Resolver {
    /// Factories get called multiple times
    ///
    /// Factories are called by the container, and they themselves will
    /// call container.resolve() as they see fit. This means we can't
    /// own a mutable borrow to the resolvers collection during the
    /// calls. Thus we must use RefCell.
    Factory(RefCell<Box<Any>>),
    Builder(Box<Any>),
    Shared(Rc<Any>),
    // TODO maybe those can be Box/RC<Any>
}

pub type DiResult<T> = Result<T, String>;
pub type ResolveResult<T> = DiResult<Rc<T>>;
type IntermediateResult = DiResult<Rc<dyn Any + 'static>>;

enum ResolverType {
    Factory,
    Builder,
    Shared,
}

impl From<&Resolver> for ResolverType {
    fn from(other: &Resolver) -> Self {
        use ResolverType::*;

        match other {
            Resolver::Factory(_) => Factory,
            Resolver::Builder(_) => Builder,
            Resolver::Shared(_) => Shared,
        }
    }
}
