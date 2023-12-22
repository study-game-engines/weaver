use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use rustc_hash::FxHashMap;

use super::{
    bundle::Bundle,
    component::Component,
    entity::Entity,
    query::{Query, Read, Write},
    system::System,
};

pub struct Components {
    pub(crate) data: FxHashMap<Entity, Vec<RefCell<Box<dyn Component>>>>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            data: FxHashMap::default(),
        }
    }

    pub fn insert<T: Component>(&mut self, entity: Entity, component: T) {
        let components = self.data.entry(entity).or_default();
        components.push(RefCell::new(Box::new(component)));
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.data.get_mut(&entity) {
            components.retain(|component| !component.borrow().as_any().is::<T>());
        }
    }
}

impl Default for Components {
    fn default() -> Self {
        Self::new()
    }
}

pub struct World {
    pub(crate) components: Components,
    systems: Vec<Arc<Mutex<dyn System>>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::new(),
            systems: Vec::new(),
        }
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    pub fn register_system<T: System>(&mut self, system: T) {
        self.systems.push(Arc::new(Mutex::new(system)));
    }

    pub fn spawn<T: Component>(&mut self, component: T) -> Entity {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let entity = Entity::new(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self.components.insert(entity, component);
        entity
    }

    pub fn build<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(self)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.components.insert(entity, component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        self.components.remove::<T>(entity);
    }

    pub fn read<T: Query>(&self) -> Read<'_> {
        let mut result = Vec::new();
        for (entity, i) in T::query(self) {
            let component = self.components.data.get(&entity).unwrap()[i].borrow();
            result.push(component);
        }
        Read { components: result }
    }

    pub fn write<T: Query>(&mut self) -> Write<'_> {
        let mut result = Vec::new();
        let query = T::query(self);
        for (entity, i) in query {
            let component = self.components.data.get(&entity).unwrap()[i].borrow_mut();
            result.push(component);
        }
        Write { components: result }
    }

    pub fn update(&mut self, delta: std::time::Duration) {
        for system in self.systems.clone().iter() {
            match system.lock() {
                Ok(mut system) => system.run(self, delta),
                Err(_) => {
                    log::warn!("Failed to acquire lock on system");
                }
            }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
