use crate::{
    prelude::Component,
    registry::{Entity, StaticId},
    storage::Data,
    world::LockedWorldHandle,
};

/// A collection of components to be added to an entity.
pub trait Bundle: Sized + 'static {
    fn type_ids() -> Vec<Entity>;
    fn into_data_vec(self, world: &LockedWorldHandle) -> Vec<Data>;
}

impl<T: Component> Bundle for T {
    fn type_ids() -> Vec<Entity> {
        vec![T::static_type_id()]
    }

    fn into_data_vec(self, world: &LockedWorldHandle) -> Vec<Data> {
        vec![T::into_data(self, world)]
    }
}

macro_rules! impl_bundle_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Component),*> Bundle for ($($name,)*) {
            fn type_ids() -> Vec<Entity> {
                vec![$(<$name as StaticId>::static_type_id()),*]
            }

            fn into_data_vec(self, world: &LockedWorldHandle) -> Vec<Data> {
                let ($($name,)*) = self;
                vec![$(<$name as Component>::into_data($name, world),)*]
            }
        }
    };
}

impl_bundle_for_tuple!(A);
impl_bundle_for_tuple!(A, B);
impl_bundle_for_tuple!(A, B, C);
impl_bundle_for_tuple!(A, B, C, D);
impl_bundle_for_tuple!(A, B, C, D, E);
impl_bundle_for_tuple!(A, B, C, D, E, F);
impl_bundle_for_tuple!(A, B, C, D, E, F, G);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H);
