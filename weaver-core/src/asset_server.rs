use crate::{
    material::Material,
    mesh::Mesh,
    renderer::{
        compute::hdr_loader::HdrLoader,
        internals::GpuResourceManager,
        pass::sky::{SKYBOX_CUBEMAP_SIZE, SKYBOX_IRRADIANCE_MAP_SIZE},
        Renderer,
    },
    texture::{NormalMapTexture, SdrTexture, Skybox, Texture, TextureFormat},
};

use std::{path::PathBuf, sync::Arc};

use rustc_hash::FxHashMap;
use weaver_ecs::{Query, World};
use weaver_proc_macro::Resource;

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]

pub struct AssetId {
    id: u64,
    load_path: PathBuf,
}

impl AssetId {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn load_path(&self) -> &PathBuf {
        &self.load_path
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self {
            id: u64::MAX,
            load_path: PathBuf::new(),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for AssetId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use path_slash::PathBufExt;
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AssetId", 2)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("load_path", &self.load_path.to_slash().unwrap())?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for AssetId {
    fn deserialize<D>(deserializer: D) -> Result<AssetId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct AssetIdHelper {
            id: u64,
            load_path: String,
        }

        let helper = AssetIdHelper::deserialize(deserializer)?;

        Ok(AssetId {
            id: helper.id,
            load_path: PathBuf::from(helper.load_path),
        })
    }
}

#[derive(Resource)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetServer {
    next_id: u64,
    path_prefix: PathBuf,
    ids: FxHashMap<PathBuf, AssetId>,
    #[cfg_attr(feature = "serde", serde(skip))]
    resource_manager: Option<Arc<GpuResourceManager>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    meshes: FxHashMap<AssetId, Mesh>,
    #[cfg_attr(feature = "serde", serde(skip))]
    textures: FxHashMap<AssetId, Texture>,
    #[cfg_attr(feature = "serde", serde(skip))]
    materials: FxHashMap<AssetId, Material>,
}

impl AssetServer {
    pub fn new(world: &World) -> anyhow::Result<Self> {
        let renderer = world.read_resource::<Renderer>()?;
        let resource_manager = renderer.resource_manager().clone();
        Ok(Self {
            next_id: 0,
            path_prefix: PathBuf::from("assets"),
            ids: FxHashMap::default(),
            resource_manager: Some(resource_manager),
            meshes: FxHashMap::default(),
            textures: FxHashMap::default(),
            materials: FxHashMap::default(),
        })
    }

    pub(crate) fn alloc_id(&mut self, path: PathBuf) -> AssetId {
        let id = AssetId {
            id: self.next_id,
            load_path: path,
        };
        self.next_id += 1;
        id
    }

    pub fn path_prefix(&self) -> &PathBuf {
        &self.path_prefix
    }

    pub fn set_path_prefix(&mut self, path_prefix: impl Into<PathBuf>) {
        self.path_prefix = path_prefix.into();
    }

    fn load_obj_mesh_with_id(
        &mut self,
        path: impl Into<PathBuf>,
        id: AssetId,
    ) -> anyhow::Result<Mesh> {
        let path = path.into();
        let mesh = Mesh::load_obj(
            path.clone(),
            self.resource_manager.as_ref().unwrap().device(),
            id.clone(),
        )?;
        self.ids.insert(path.clone(), id.clone());
        self.meshes.insert(id.clone(), mesh);
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.meshes.get(id))
            .unwrap()
            .clone())
    }

    fn load_gltf_mesh_with_id(
        &mut self,
        path: impl Into<PathBuf>,
        id: AssetId,
    ) -> anyhow::Result<Mesh> {
        let path = path.into();
        let mesh = Mesh::load_gltf(
            path.clone(),
            self.resource_manager.as_ref().unwrap().device(),
            id.clone(),
        )?;
        self.ids.insert(path.clone(), id.clone());
        self.meshes.insert(id.clone(), mesh);
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.meshes.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_mesh(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Mesh> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id(path.clone());
            if path.extension().unwrap() == "obj" {
                return self.load_obj_mesh_with_id(path.clone(), id.clone());
            } else {
                return self.load_gltf_mesh_with_id(path.clone(), id.clone());
            }
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.meshes.get(id))
            .unwrap()
            .clone())
    }

    fn load_material_with_id(
        &mut self,
        path: impl Into<PathBuf>,
        id: AssetId,
    ) -> anyhow::Result<Material> {
        let path = path.into();
        let mut materials = Material::load_gltf(path.clone(), id.clone())?;
        self.ids.insert(path.clone(), id.clone());
        self.materials.insert(id, materials.remove(0));
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.materials.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_material(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Material> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };
        if !self.ids.contains_key(&path) {
            let id = self.alloc_id(path.clone());
            return self.load_material_with_id(path.clone(), id.clone());
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.materials.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_texture(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Texture> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id(path.clone());
            let texture = Texture::load(path.clone(), SdrTexture::FORMAT, None);
            self.ids.insert(path.clone(), id.clone());
            self.textures.insert(id.clone(), texture);
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.textures.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_normal_map(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Texture> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id(path.clone());
            let texture = Texture::load(path.clone(), NormalMapTexture::FORMAT, None);
            self.ids.insert(path.clone(), id.clone());
            self.textures.insert(id, texture);
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.textures.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_skybox(
        &mut self,
        path: impl Into<PathBuf>,
        hdr_loader: &HdrLoader,
    ) -> anyhow::Result<Skybox> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };
        let texture = hdr_loader.load(
            self.resource_manager.as_ref().unwrap(),
            SKYBOX_CUBEMAP_SIZE,
            path,
        )?;
        let irradiance = hdr_loader.generate_irradiance_map(
            self.resource_manager.as_ref().unwrap(),
            &texture,
            SKYBOX_IRRADIANCE_MAP_SIZE,
        )?;
        Ok(Skybox::new(texture, irradiance))
    }

    pub fn load_all_assets(&mut self, world: &World) -> anyhow::Result<()> {
        // locate all the assets in the world
        {
            let query = world.query::<&mut Mesh, ()>();
            for mut mesh in query.iter() {
                let id = mesh.asset_id().clone();
                let path = id.load_path().clone();
                let loaded = if let Some(extension) = path.extension() {
                    if extension == "obj" {
                        self.load_obj_mesh_with_id(path, id)?
                    } else {
                        self.load_gltf_mesh_with_id(path, id)?
                    }
                } else {
                    self.load_gltf_mesh_with_id(path.clone(), id.clone())
                        .unwrap_or_else(|_| {
                            self.load_obj_mesh_with_id(path.clone(), id)
                                .unwrap_or_else(|_| {
                                    panic!("Failed to load mesh at path: {:?}", path);
                                })
                        })
                };
                *mesh = loaded;
            }
        }

        {
            let query = world.query::<&mut Material, ()>();
            for mut material in query.iter() {
                let id = material.asset_id().clone();
                let path = id.load_path().clone();
                let loaded = self.load_material_with_id(path, id)?;
                *material = loaded;
            }
        }

        Ok(())
    }
}
