//! Narrow typed boundary between historical domain logic and the map rendering engine.

use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;

use crate::{navigation::Camera, scene::renderer::RenderError};

/// Display projection independent of the underlying rendering engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayProjection {
    /// Perspective view of a spherical planetary body at both global and local scales.
    VerticalPerspective,
    /// Spherical globe projection.
    Globe,
    /// Planar Web Mercator projection.
    Mercator,
}

impl DisplayProjection {
    pub(super) const fn into_maplibre(self) -> maplibre::projection::ProjectionType {
        match self {
            Self::VerticalPerspective => maplibre::projection::ProjectionType::VerticalPerspective,
            Self::Globe => maplibre::projection::ProjectionType::Globe,
            Self::Mercator => maplibre::projection::ProjectionType::Mercator,
        }
    }
}

/// Pointer facts for one GUI frame over the map viewport, in logical pixels from its top-left corner.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportPointer {
    /// Logical pixel offset inside the map viewport, if hovered or interacting.
    pub pixel: Option<egui::Vec2>,
    /// Primary pointer button pressed during this frame.
    pub pressed: bool,
    /// Primary pointer button released during this frame.
    pub released: bool,
    /// Pointer is hovering inside the map viewport.
    pub hovered: bool,
}

impl ViewportPointer {
    /// Sample pointer state from an egui map response.
    pub fn read(ui: &egui::Ui, response: &egui::Response) -> Self {
        let (pressed, released) = ui.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_released(),
            )
        });
        Self {
            pixel: response
                .interact_pointer_pos()
                .or_else(|| response.hover_pos())
                .map(|position| position - response.rect.min),
            pressed,
            released,
            hovered: response.hovered(),
        }
    }
}

/// Engine-neutral interface for updating, picking, and rendering the planetary scene.
pub trait ScenePort {
    /// Refresh the visible GeoJSON feature collection and switch planetary body layers.
    fn update(&mut self, points: serde_json::Value, body: BodyId) -> Result<(), RenderError>;

    /// Highlight the selected entity and optional auxiliary network features.
    fn select(
        &mut self,
        id: Option<EntityId>,
        extra: Vec<serde_json::Value>,
    ) -> Result<(), RenderError>;

    /// Render a frame at the viewport's logical size and pixel ratio, returning the shared texture.
    fn frame(
        &mut self,
        size: egui::Vec2,
        pixels_per_point: f32,
        camera: Option<Camera>,
    ) -> Result<egui::TextureId, RenderError>;

    /// Return every distinct entity beneath a logical viewport pixel.
    fn pick(&self, pixel: [f64; 2]) -> Result<Vec<EntityId>, RenderError>;

    /// Geographic `[west, south, east, north]` envelope for streaming regional detail.
    fn detail_bounds(&self) -> Option<[f64; 4]>;

    /// Route pointer state to the engine's navigation HUD; returns true when the HUD owns the interaction.
    fn pointer_controls(&mut self, pointer: ViewportPointer) -> bool;

    /// Advance HUD camera animations and smooth zooms; returns true when another frame is needed.
    fn advance_controls(&mut self) -> bool;

    /// Rotate the globe between two logical viewport positions.
    fn drag(&mut self, from: egui::Vec2, to: egui::Vec2) -> bool;

    /// Orbit bearing and pitch by logical pixel delta.
    fn orbit(&mut self, delta: egui::Vec2) -> Result<bool, RenderError>;

    /// Zoom about a logical viewport pixel while keeping ground anchor fixed.
    fn zoom(&mut self, pixel: egui::Vec2, delta: f64) -> bool;

    /// Whether background tile or animation work requests another GUI frame.
    fn needs_redraw(&self) -> bool;
}
