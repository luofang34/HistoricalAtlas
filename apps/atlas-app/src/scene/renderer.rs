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
    hud::NavigationHudPlugin,
    raster::{DefaultRasterTransferables, RasterPlugin},
    render::{
        RenderPlugin,
        host_gpu::HostGpu,
        settings::RendererSettings,
        view_state::{CameraOptions, NavigationMode},
    },
    vector::{DefaultVectorTransferables, VectorPlugin},
    window::PhysicalSize,
};
use thiserror::Error;

/// A map operation failed without changing the evidence ledger.
#[derive(Debug, Error)]
pub enum RenderError {
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
    /// Globe camera projection is unavailable.
    #[error("map projection: {0}")]
    Projection(#[from] maplibre::render::projection::ProjectionStateError),
    /// Map feature identity is invalid.
    #[error("map feature identity: {0}")]
    Identity(#[from] serde_json::Error),
}

/// Map and the texture binding used by egui.
pub struct MapRenderer {
    map: HeadlessMap,
    gpu: RenderState,
    texture: egui::TextureId,
    raster_layers: Vec<(String, BodyId)>,
    terrain: Vec<(
        BodyId,
        Option<maplibre::style::terrain::TerrainSpecification>,
    )>,
    dimensions: [u32; 2],
    picking: super::picking::Picking,
    selection_picking: super::picking::Picking,
    /// When the map started; the frame clock is the time since, so animations and
    /// tile retry backoffs advance with real time.
    started: web_time::Instant,
}

impl MapRenderer {
    /// Initialize the map on an existing application GPU.
    pub fn new(
        gpu: RenderState,
        style: maplibre::style::Style,
        basemap: &super::basemap::Manifest,
    ) -> Result<Self, RenderError> {
        let host = HostGpu {
            instance: gpu.instance.clone(),
            adapter: gpu.adapter.clone(),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        };
        let (kernel, renderer) = create_headless_renderer_on_host_gpu(
            1,
            1,
            host,
            RendererSettings::default(),
            Some(super::basemap::loader(basemap)),
        )?;
        let mut map = HeadlessMap::new(
            style,
            renderer,
            kernel,
            vec![
                Box::new(RenderPlugin),
                Box::new(BackgroundPlugin),
                Box::new(RasterPlugin::<DefaultRasterTransferables>::default()),
                Box::new(maplibre::hillshade::HillshadePlugin),
                Box::new(VectorPlugin::<DefaultVectorTransferables>::default()),
                // Registered for every body: switching bodies sets or clears the style's
                // terrain at run time, and the plugin idles while there is none.
                Box::new(maplibre::terrain::TerrainPlugin::<
                    maplibre::terrain::DefaultDemTransferables,
                >::default()),
                Box::new(NavigationHudPlugin),
            ],
        )?;
        map.set_navigation_mode(NavigationMode::FreeGlobe)?;
        // Terrain is read by looking across it, as in MapLibre's 3D terrain example.
        map.set_max_pitch(cgmath::Deg(85.0));
        map.render_source_frames(ProcessedLayers::default(), Vec::new(), 1)?;
        let view = map.head_texture_view().ok_or(RenderError::Texture)?;
        let texture = gpu.renderer.write().register_native_texture(
            &gpu.device,
            view,
            wgpu::FilterMode::Linear,
        );
        Ok(Self {
            started: web_time::Instant::now(),
            map,
            gpu,
            texture,
            raster_layers: basemap.layers(),
            terrain: [BodyId::Earth, BodyId::Moon, BodyId::Mars]
                .into_iter()
                .map(|body| {
                    let spec = basemap
                        .terrain(body)
                        .and_then(|value| serde_json::from_value(value).ok());
                    (body, spec)
                })
                .collect(),
            dimensions: [1, 1],
            picking: Default::default(),
            selection_picking: Default::default(),
        })
    }

    /// Refresh the visible point collection.
    pub fn update(&mut self, points: serde_json::Value, body: BodyId) -> Result<(), RenderError> {
        for (id, raster_body) in &self.raster_layers {
            let visibility = if *raster_body == body {
                "visible"
            } else {
                "none"
            };
            self.map.mutate_style(|style| {
                style.set_layout_property(id, "visibility", visibility.into())
            })?;
        }
        let terrain = self
            .terrain
            .iter()
            .find(|(candidate, _)| *candidate == body)
            .and_then(|(_, spec)| spec.clone());
        self.map.mutate_style(|style| {
            if style.terrain != terrain {
                style.terrain = terrain;
            }
            Ok(maplibre::style::mutation::StyleChange::default())
        })?;
        let picking = super::picking::Picking::new(&points)?;
        self.map.set_geojson_source("observations", points)?;
        self.picking = picking;
        Ok(())
    }

    /// Render at the viewport's physical size and return the shared texture.
    pub fn frame(
        &mut self,
        size: egui::Vec2,
        ratio: f32,
        camera: Option<Camera>,
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
        if let Some(camera) = camera {
            self.map.jump_to(CameraOptions {
                center: Some(LatLon::new(camera.center.latitude, camera.center.longitude)),
                zoom: Some(camera.zoom),
                bearing: Some(cgmath::Deg(0.0)),
                pitch: Some(cgmath::Deg(0.0)),
                roll: Some(cgmath::Deg(0.0)),
                ..Default::default()
            })?;
        }
        self.map.frame_input_mut().timestamp = self.started.elapsed();
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

    /// Rotate the globe between two viewport positions in logical pixels.
    pub fn drag(&mut self, from: egui::Vec2, to: egui::Vec2) -> bool {
        self.map.drag_globe(
            cgmath::Point2::new(f64::from(from.x), f64::from(from.y)),
            cgmath::Point2::new(f64::from(to.x), f64::from(to.y)),
        )
    }

    /// Zoom with the ground under the pointer held in place.
    pub fn zoom(&mut self, pixel: egui::Vec2, delta: f64) -> bool {
        self.map.zoom_globe(
            cgmath::Point2::new(f64::from(pixel.x), f64::from(pixel.y)),
            delta,
        )
    }

    /// Return every distinct entity beneath the pointer, including coincident villages.
    pub fn pick(&self, [x, y]: [f64; 2]) -> Result<Vec<EntityId>, RenderError> {
        let camera = maplibre::render::projection::globe_camera_for_view(self.map.view_state())?;
        let pixel = cgmath::Point2::new(x, y);
        let mut hits = self.picking.pick(&camera, pixel);
        hits.extend(self.selection_picking.pick(&camera, pixel));
        hits.sort();
        hits.dedup();
        Ok(hits)
    }

    /// Update only the selected entity and its optional territory network.
    pub fn select(
        &mut self,
        id: Option<EntityId>,
        extra: Vec<serde_json::Value>,
    ) -> Result<(), RenderError> {
        let selection = self.picking.selection(id, extra);
        let picking = super::picking::Picking::new(&selection)?;
        self.map.set_geojson_source("selection", selection)?;
        self.selection_picking = picking;
        Ok(())
    }

    /// Geographic envelope for loading regional detail, unwrapped around the camera longitude.
    pub fn detail_bounds(&self) -> Option<[f64; 4]> {
        let view = self.map.view_state();
        if view.zoom().value() < 4.0 {
            return None;
        }
        let camera = maplibre::render::projection::globe_camera_for_view(view).ok()?;
        let (width, height) = view.viewport_size();
        let center = camera.center();
        let mut bounds = [
            center.longitude,
            center.latitude,
            center.longitude,
            center.latitude,
        ];
        for x in [0., 0.5, 1.] {
            for y in [0., 0.5, 1.] {
                if let Some(point) =
                    camera.screen_point_to_location(cgmath::Point2::new(x * width, y * height))
                {
                    let longitude = center.longitude
                        + (point.longitude - center.longitude + 180.).rem_euclid(360.)
                        - 180.;
                    bounds = [
                        bounds[0].min(longitude),
                        bounds[1].min(point.latitude),
                        bounds[2].max(longitude),
                        bounds[3].max(point.latitude),
                    ];
                }
            }
        }
        if bounds[1] < -80. || bounds[3] > 80. {
            bounds[0] = center.longitude - 180.;
            bounds[2] = center.longitude + 180.;
        }
        Some(bounds)
    }

    /// Whether map work requests another GUI frame.
    pub fn needs_redraw(&self) -> bool {
        self.map.needs_redraw()
    }
}

mod navigation;

impl Drop for MapRenderer {
    fn drop(&mut self) {
        self.gpu.renderer.write().free_texture(&self.texture);
    }
}
