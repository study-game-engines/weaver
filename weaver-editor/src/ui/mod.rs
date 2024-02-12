use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use fabricate::commands::Commands;
use weaver::prelude::*;

use crate::state::EditorState;

use self::fps_counter::FpsDisplay;

pub mod component_inspector;
pub mod fps_counter;
pub mod scene_tree;
pub mod script;
pub mod syntax_highlighting;

pub type Tab = String;

#[derive(Clone, Component)]
pub struct Tabs {
    pub(crate) tree: DockState<Tab>,
}

impl Default for Tabs {
    fn default() -> Self {
        let mut tree = DockState::new(vec![]);
        let [left, right] = tree.main_surface_mut().split_left(
            NodeIndex::root(),
            0.3,
            vec!["Scene Tree".to_string()],
        );

        tree.main_surface_mut()
            .split_below(right, 0.5, vec!["Component Inspector".to_string()]);

        let [top, _bottom] =
            tree.main_surface_mut()
                .split_above(left, 0.3, vec!["Assets".to_string()]);

        tree.main_surface_mut()
            .split_above(right, 0.3, vec!["Console".to_string()]);

        tree.main_surface_mut()
            .split_above(top, 1.0, vec!["Viewport".to_string()]);

        Self { tree }
    }
}

pub struct EditorTabViewer<'a> {
    world: LockedWorldHandle,
    state: &'a mut EditorState,
}

impl<'a> TabViewer for EditorTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.world
            .defer(|world, commands| {
                let renderer = world.read_resource::<Renderer>().unwrap();
                match tab.as_str() {
                    "Viewport" => {
                        let camera = world
                            .query()
                            .write::<FlyCameraController>()
                            .unwrap()
                            .build();
                        let mut camera = camera.iter().next().unwrap();
                        let rect = ui.min_rect();
                        renderer.set_viewport_rect(rect.into());
                        camera.get_mut::<FlyCameraController>().unwrap().aspect =
                            rect.aspect_ratio();

                        if let Some(id) = self.state.viewport_id {
                            let ctx = world.read_resource::<EguiContext>().unwrap();

                            let view = renderer
                                .main_viewport()
                                .read()
                                .color_view(renderer.resource_manager());

                            ctx.update_texture(renderer.device(), &view, id);

                            ui.image((id, rect.size()));
                        }
                    }
                    "Scene Tree" => {
                        scene_tree::scene_tree_ui(&self.world, self.state, ui);
                    }
                    "Component Inspector" => {
                        component_inspector::component_inspector_ui(
                            world, commands, self.state, ui,
                        );
                    }
                    tab => {
                        ui.label(tab);
                    }
                }
            })
            .unwrap();
    }
}

pub fn editor_ui(world: &World, commands: &mut Commands) -> anyhow::Result<()> {
    let mut state = world.write_resource::<EditorState>().unwrap();
    let mut tree = world.write_resource::<Tabs>().unwrap();
    let mut fps = world.write_resource::<FpsDisplay>().unwrap();
    let ctx = world.read_resource::<EguiContext>().unwrap();
    ctx.draw_if_ready(|ctx| {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            fps.run_ui(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            DockArea::new(&mut tree.tree)
                .style(Style::from_egui(ctx.style().as_ref()))
                .show_inside(
                    ui,
                    &mut EditorTabViewer {
                        world: commands.world().clone(),
                        state: &mut state,
                    },
                );
        });

        state.rename_entity_window(commands.world(), ctx).unwrap();
    });

    Ok(())
}
