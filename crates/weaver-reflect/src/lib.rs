use weaver_util::prelude::{impl_downcast, Downcast};

pub mod impls;
pub mod registry;

pub mod prelude {
    pub use crate::registry::*;
    pub use crate::Reflect;
    pub use weaver_reflect_macros::*;
}

pub trait Reflect: Downcast {
    fn as_reflect(&self) -> &dyn Reflect;
    fn as_reflect_mut(&mut self) -> &mut dyn Reflect;
    fn into_reflect_box(self: Box<Self>) -> Box<dyn Reflect>;

    fn reflect_type_name(&self) -> &'static str;
}
impl_downcast!(Reflect);

impl<T: Reflect> Reflect for Box<T> {
    fn as_reflect(&self) -> &dyn Reflect {
        self.as_ref()
    }

    fn as_reflect_mut(&mut self) -> &mut dyn Reflect {
        self.as_mut()
    }

    fn into_reflect_box(self: Box<Self>) -> Box<dyn Reflect> {
        self
    }

    fn reflect_type_name(&self) -> &'static str {
        T::reflect_type_name(self)
    }
}

impl dyn Reflect {
    pub fn take<T: Reflect>(self: Box<dyn Reflect>) -> Result<T, Box<dyn Reflect>> {
        self.downcast::<T>().map(|boxed| *boxed)
    }
}

#[cfg(test)]
mod tests {
    use crate as weaver_reflect;
    use registry::{Struct, TypeInfo, TypeRegistry};
    use weaver_reflect_macros::Reflect;

    use super::*;

    #[test]
    fn test_reflect() {
        let value = 42u8;
        let reflect = &value as &dyn Reflect;
        assert_eq!(reflect.reflect_type_name(), "u8");

        #[derive(Reflect)]
        struct TestStruct {
            value: u8,
        }

        let value = TestStruct { value: 42 };
        let reflect = &value as &dyn Reflect;
        assert_eq!(reflect.reflect_type_name(), "TestStruct");
        assert_eq!(
            reflect.as_any().downcast_ref::<TestStruct>().unwrap().value,
            42
        );
        assert_eq!(
            value
                .field("value")
                .unwrap()
                .as_any()
                .downcast_ref::<u8>()
                .unwrap(),
            &42
        );
    }

    #[test]
    fn test_registry() {
        #[derive(Reflect)]
        struct TestStruct {
            value: u8,
        }

        let mut registry = TypeRegistry::new();
        registry.register::<TestStruct>();

        let type_registration = registry.get_type_info::<TestStruct>().unwrap();
        let TypeInfo::Struct(info) = type_registration.type_info else {
            panic!(
                "Expected TypeInfo::Struct, got {:?}",
                type_registration.type_info
            );
        };

        assert_eq!(info.type_name, "TestStruct");
        assert_eq!(info.fields.len(), 1);
        assert_eq!(info.field("value").unwrap().name, "value");
        assert_eq!(info.field("value").unwrap().type_name, "u8");
        assert_eq!(
            info.field("value").unwrap().type_id,
            std::any::TypeId::of::<u8>()
        );
        assert!(info.field("missing").is_none());
    }
}
