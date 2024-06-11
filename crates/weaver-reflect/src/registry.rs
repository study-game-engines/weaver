use std::{
    any::TypeId,
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

use weaver_ecs::prelude::Component;
use weaver_util::prelude::{impl_downcast, Downcast};

use crate::Reflect;

pub trait Typed: Reflect {
    fn type_name() -> &'static str;
    fn type_info() -> &'static TypeInfo;
}

#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructInfo),
    List(ListInfo),
    Map(MapInfo),
    Value(ValueInfo),
}

#[derive(Debug, Clone)]
pub struct ValueInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
}

pub trait Struct: Reflect {
    fn field(&self, field_name: &str) -> Option<&dyn Reflect>;
    fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn Reflect>;
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub fields: Box<[FieldInfo]>,
    pub field_names: Box<[&'static str]>,
    pub field_indices: HashMap<&'static str, usize>,
}

impl StructInfo {
    pub fn new<T: Reflect + Typed>(fields: &[FieldInfo]) -> Self {
        let type_id = TypeId::of::<T>();
        let type_name = T::type_name();
        let field_names: Box<[&'static str]> = fields.iter().map(|field| field.name).collect();
        let field_indices = field_names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, i))
            .collect();
        Self {
            type_id,
            type_name,
            fields: fields.into(),
            field_names,
            field_indices,
        }
    }

    pub fn field(&self, field_name: &str) -> Option<&FieldInfo> {
        self.field_index(field_name)
            .map(|index| &self.fields[index])
    }

    pub fn field_index(&self, field_name: &str) -> Option<usize> {
        self.field_indices.get(field_name).copied()
    }

    pub fn is<T: Reflect + Typed>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: &'static str,
    pub type_name: &'static str,
    pub type_id: TypeId,
}

pub trait List: Reflect {
    fn len_reflect(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len_reflect() == 0
    }
    fn get_reflect(&self, index: usize) -> Option<&dyn Reflect>;
    fn get_mut_reflect(&mut self, index: usize) -> Option<&mut dyn Reflect>;
    fn insert_reflect(&mut self, index: usize, value: Box<dyn Reflect>);
    fn push_reflect(&mut self, value: Box<dyn Reflect>) {
        self.insert_reflect(self.len_reflect(), value);
    }
    fn remove_reflect(&mut self, index: usize) -> Option<Box<dyn Reflect>>;
    fn clear_reflect(&mut self);
    fn pop_reflect(&mut self) -> Option<Box<dyn Reflect>> {
        if self.is_empty() {
            None
        } else {
            self.remove_reflect(self.len_reflect() - 1)
        }
    }
    fn drain_reflect(self: Box<Self>) -> Vec<Box<dyn Reflect>>;
}

#[derive(Debug, Clone)]
pub struct ListInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub item_type_id: TypeId,
    pub item_type_name: &'static str,
}

impl ListInfo {
    pub fn new<L: Reflect + Typed, I: Reflect + Typed>() -> Self {
        Self {
            type_id: TypeId::of::<L>(),
            type_name: L::type_name(),
            item_type_id: TypeId::of::<I>(),
            item_type_name: I::type_name(),
        }
    }

    pub fn is<L: Reflect + Typed>(&self) -> bool {
        self.type_id == TypeId::of::<L>()
    }

    pub fn item_is<I: Reflect + Typed>(&self) -> bool {
        self.item_type_id == TypeId::of::<I>()
    }
}

pub trait Map: Reflect {
    fn len_reflect(&self) -> usize;
    fn is_empty_reflect(&self) -> bool {
        self.len_reflect() == 0
    }
    fn get_reflect(&self, key: &dyn Reflect) -> Option<&dyn Reflect>;
    fn get_mut_reflect(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect>;
    fn insert_reflect(&mut self, key: Box<dyn Reflect>, value: Box<dyn Reflect>);
    fn remove_reflect(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>>;
    fn clear_reflect(&mut self);
}

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub key_type_id: TypeId,
    pub key_type_name: &'static str,
    pub value_type_id: TypeId,
    pub value_type_name: &'static str,
}

impl MapInfo {
    pub fn new<M: Reflect + Typed, K: Reflect + Typed, V: Reflect + Typed>() -> Self {
        Self {
            type_id: TypeId::of::<M>(),
            type_name: M::type_name(),
            key_type_id: TypeId::of::<K>(),
            key_type_name: K::type_name(),
            value_type_id: TypeId::of::<V>(),
            value_type_name: V::type_name(),
        }
    }

    pub fn is<M: Reflect + Typed>(&self) -> bool {
        self.type_id == TypeId::of::<M>()
    }

    pub fn key_is<K: Reflect + Typed>(&self) -> bool {
        self.key_type_id == TypeId::of::<K>()
    }

    pub fn value_is<V: Reflect + Typed>(&self) -> bool {
        self.value_type_id == TypeId::of::<V>()
    }
}

pub trait TypeAuxData: Downcast {
    fn clone_type_aux_data(&self) -> Box<dyn TypeAuxData>;
}
impl_downcast!(TypeAuxData);

impl<T: Clone + Downcast> TypeAuxData for T {
    fn clone_type_aux_data(&self) -> Box<dyn TypeAuxData> {
        Box::new(self.clone())
    }
}

pub trait FromType<T> {
    fn from_type() -> Box<Self>;
}

#[derive(Default)]
pub struct TypeIdHasher {
    state: u64,
}

impl Hasher for TypeIdHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write_u128(&mut self, i: u128) {
        self.state = i as u64;
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i;
    }

    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!("TypeIdHasher should not be used with anything other than TypeId")
    }
}

pub type TypeIdMap<T> =
    std::collections::hash_map::HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;

pub struct TypeRegistration {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub type_info: &'static TypeInfo,
    pub type_aux_data: TypeIdMap<Box<dyn TypeAuxData>>,
}

#[derive(Component)]
pub struct TypeRegistry {
    types: TypeIdMap<TypeRegistration>,
    type_names: HashMap<&'static str, TypeId>,
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

impl TypeRegistry {
    pub fn empty() -> Self {
        Self {
            types: TypeIdMap::default(),
            type_names: HashMap::new(),
        }
    }

    pub fn new() -> Self {
        let mut registry = Self::empty();
        registry.register::<u8>();
        registry.register::<u16>();
        registry.register::<u32>();
        registry.register::<u64>();
        registry.register::<u128>();
        registry.register::<usize>();
        registry.register::<i8>();
        registry.register::<i16>();
        registry.register::<i32>();
        registry.register::<i64>();
        registry.register::<i128>();
        registry.register::<isize>();
        registry.register::<f32>();
        registry.register::<f64>();
        registry.register::<bool>();
        registry.register::<String>();
        registry
    }

    pub fn register<T: Typed>(&mut self) {
        if self.types.contains_key(&TypeId::of::<T>()) {
            return;
        }
        let type_registration = TypeRegistration {
            type_id: TypeId::of::<T>(),
            type_name: T::type_name(),
            type_info: T::type_info(),
            type_aux_data: TypeIdMap::default(),
        };

        self.type_names
            .insert(type_registration.type_name, type_registration.type_id);
        self.types
            .insert(type_registration.type_id, type_registration);
    }

    pub fn get_type_info<T: Reflect>(&self) -> Option<&TypeRegistration> {
        self.get_type_info_by_id(TypeId::of::<T>())
    }

    pub fn get_type_info_by_id(&self, type_id: TypeId) -> Option<&TypeRegistration> {
        self.types.get(&type_id)
    }

    pub fn get_type_info_by_name(&self, type_name: &str) -> Option<&TypeRegistration> {
        self.type_names
            .get(type_name)
            .and_then(|type_id| self.types.get(type_id))
    }

    pub fn get_type_data<T: Reflect, D: TypeAuxData>(&self) -> Option<&D> {
        self.get_type_data_by_id(TypeId::of::<T>())
    }

    pub fn get_type_data_by_id<D: TypeAuxData>(&self, type_id: TypeId) -> Option<&D> {
        self.types.get(&type_id).and_then(|type_registration| {
            type_registration
                .type_aux_data
                .get(&type_id)
                .and_then(|type_aux_data| type_aux_data.downcast_ref())
        })
    }

    pub fn register_type_data<T: Reflect, D: TypeAuxData + FromType<T>>(&mut self) {
        let type_id = TypeId::of::<T>();
        let type_registration = self.types.get_mut(&type_id).unwrap();
        type_registration
            .type_aux_data
            .insert(type_id, D::from_type());
    }
}