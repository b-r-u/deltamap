use cgmath::Vector2;
use mercator_view::MercatorView;
use orthografic_view::OrthograficView;


/// A view of the map with a specific projection.
#[derive(Clone, Debug)]
pub enum ProjectionView {
    Mercator(MercatorView),
    Orthografic(OrthograficView),
}

impl ProjectionView {
    pub fn viewport_size(&self) -> Vector2<f64> {
        match *self {
            ProjectionView::Mercator(ref merc) => merc.viewport_size,
            ProjectionView::Orthografic(ref ortho) => ortho.viewport_size,
        }
    }

    pub fn tile_size(&self) -> u32 {
        match *self {
            ProjectionView::Mercator(ref merc) => merc.tile_size,
            ProjectionView::Orthografic(ref ortho) => ortho.tile_size,
        }
    }
}
