//! MapLibre draws into a texture on egui's GPU device.

use crate::navigation::Camera;
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;
use egui_wgpu::RenderState;
use maplibre::{
    background::BackgroundPlugin,
    coords::LatLon,
    headless::{
        create_headless_renderer_on_host_gpu,
        map::{HeadlessMap, ProcessedLayers},
    },
    raster::{DefaultRasterTransferables, RasterPlugin},
    render::{
        RenderPlugin, host_gpu::HostGpu, settings::RendererSettings, view_state::CameraOptions,
    },
    sdf::query::{QueryGeometry, QueryOptions},
    vector::{DefaultVectorTransferables, VectorPlugin},
    window::PhysicalSize,
};
use thiserror::Error;

/// A map operation failed without changing the evidence ledger.
#[derive(Debug, Error)]
pub enum RenderError {
    /// Bundled physical tile could not be decoded.
    #[error("physical basemap image: {0}")]
    Image(#[from] image::ImageError),
    /// A body-specific layer could not be changed.
    #[error("map style: {0}")]
    Style(#[from] maplibre::style::mutation::StyleMutationError),
    /// Shared renderer initialization failed.
    #[error("map GPU initialization: {0}")]
    Renderer(#[from] maplibre::headless::HeadlessRendererError),
    /// Map plugin initialization failed.
    #[error("map initialization: {0}")]
    Map(#[from] maplibre::map::MapError),
    /// Offscreen texture is unavailable.
    #[error("map color texture is unavailable")]
    Texture,
    /// Source replacement failed.
    #[error("map source update: {0}")]
    Source(#[from] maplibre::geojson::update::SourceUpdateError),
    /// Frame execution failed.
    #[error("map frame: {0}")]
    Frame(#[from] maplibre::headless::map::HeadlessMapOperationError),
    /// Camera position is invalid.
    #[error("map camera: {0}")]
    Camera(#[from] maplibre::render::view_state::NavigationError),
    /// Feature picking failed.
    #[error("map selection: {0}")]
    Query(#[from] maplibre::sdf::query::QueryError),
    /// Map feature identity is invalid.
    #[error("map feature identity: {0}")]
    Identity(#[from] serde_json::Error),
}

/// Map and the texture binding used by egui.
pub struct MapRenderer {
    map: HeadlessMap,
    gpu: RenderState,
    texture: egui::TextureId,
    dimensions: [u32; 2],
}

impl MapRenderer {
    /// Initialize the map on an existing application GPU.
    pub fn new(gpu: RenderState, style: maplibre::style::Style) -> Result<Self, RenderError> {
        let host = HostGpu {
            instance: gpu.instance.clone(),
            adapter: gpu.adapter.clone(),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        };
        let (kernel, renderer) =
            create_headless_renderer_on_host_gpu(1, 1, host, RendererSettings::default(), None)?;
        let mut map = HeadlessMap::new(
            style,
            renderer,
            kernel,
            vec![
                Box::new(RenderPlugin),
                Box::new(BackgroundPlugin),
                Box::new(RasterPlugin::<DefaultRasterTransferables>::default()),
                Box::new(VectorPlugin::<DefaultVectorTransferables>::default()),
            ],
        )?;
        map.render_source_frames(ProcessedLayers::default(), super::basemap::tiles()?, 1)?;
        let view = map.head_texture_view().ok_or(RenderError::Texture)?;
        let texture = gpu.renderer.write().register_native_texture(
            &gpu.device,
            view,
            wgpu::FilterMode::Linear,
        );
        Ok(Self {
            map,
            gpu,
            texture,
            dimensions: [1, 1],
        })
    }

    /// Refresh the visible point collection.
    pub fn update(&mut self, points: serde_json::Value, body: BodyId) -> Result<(), RenderError> {
        let visibility = if body == BodyId::Earth {
            "visible"
        } else {
            "none"
        };
        for id in ["natural-earth", "terrain"] {
            self.map.mutate_style(|style| {
                style.set_layout_property(id, "visibility", visibility.into())
            })?;
        }
        Ok(self.map.set_geojson_source("observations", points)?)
    }

    /// Render at the viewport's physical size and return the shared texture.
    pub fn frame(
        &mut self,
        size: egui::Vec2,
        ratio: f32,
        camera: Camera,
    ) -> Result<egui::TextureId, RenderError> {
        let limit = self.gpu.device.limits().max_texture_dimension_2d;
        let dimensions = [
            (size.x * ratio).round().clamp(1.0, limit as f32) as u32,
            (size.y * ratio).round().clamp(1.0, limit as f32) as u32,
        ];
        let resized = dimensions != self.dimensions;
        if resized {
            let [width, height] = dimensions;
            let physical = PhysicalSize::new(width, height).ok_or(RenderError::Texture)?;
            self.map.resize(physical);
            self.dimensions = dimensions;
        }
        self.map.set_pixel_ratio(f64::from(ratio));
        self.map.jump_to(CameraOptions {
            center: Some(LatLon::new(camera.center.latitude, camera.center.longitude)),
            zoom: Some(camera.zoom),
            ..Default::default()
        })?;
        self.map.run_frame()?;
        // MapLibre replaces resized attachments during frame preparation.
        if resized {
            let view = self.map.head_texture_view().ok_or(RenderError::Texture)?;
            self.gpu
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    &self.gpu.device,
                    view,
                    wgpu::FilterMode::Linear,
                    self.texture,
                );
        }
        Ok(self.texture)
    }

    /// Return every distinct entity beneath the pointer, including coincident villages.
    pub fn pick(&self, [x, y]: [f64; 2]) -> Result<Vec<EntityId>, RenderError> {
        let geometry = QueryGeometry::Box {
            min: [x - 5.0, y - 5.0],
            max: [x + 5.0, y + 5.0],
        };
        let features = self.map.query_rendered_features(
            geometry,
            &QueryOptions {
                layers: Some(vec![
                    "observations".into(),
                    "mapped-places".into(),
                    "external-events".into(),
                    "territory-nodes".into(),
                    "boundaries".into(),
                    "boundary-lines".into(),
                ]),
                ..Default::default()
            },
        )?;
        let mut ids = std::collections::BTreeSet::new();
        for feature in features {
            if let Some(value) = feature.properties.get("entity_id") {
                ids.insert(serde_json::from_value(value.clone())?);
            }
        }
        Ok(ids.into_iter().collect())
    }

    /// Whether map work requests another GUI frame.
    pub fn needs_redraw(&self) -> bool {
        self.map.needs_redraw()
    }
}

impl Drop for MapRenderer {
    fn drop(&mut self) {
        self.gpu.renderer.write().free_texture(&self.texture);
    }
}
