use crate::ui::{Graphic, SampleStrat, Transform, Ui};
use common::{
    assets::{self, AssetExt, AssetHandle, DotVoxAsset},
    comp::item::item_key::ItemKey,
    figure::Segment,
};
use conrod_core::image::Id;
use hashbrown::HashMap;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use vek::*;

pub fn animate_by_pulse(ids: &[Id], pulse: f32) -> Id {
    let animation_frame = (pulse * 3.0) as usize;
    ids[animation_frame % ids.len()]
}

#[derive(Serialize, Deserialize)]
enum ImageSpec {
    Png(String),
    Vox(String),
    // (specifier, offset, (axis, 2 * angle / pi), zoom)
    VoxTrans(String, [f32; 3], [f32; 3], f32),
}
impl ImageSpec {
    fn create_graphic(&self) -> Graphic {
        match self {
            ImageSpec::Png(specifier) => Graphic::Image(graceful_load_img(specifier), None),
            ImageSpec::Vox(specifier) => Graphic::Voxel(
                graceful_load_segment_no_skin(specifier),
                Transform {
                    stretch: false,
                    ..Default::default()
                },
                SampleStrat::None,
            ),
            ImageSpec::VoxTrans(specifier, offset, [rot_x, rot_y, rot_z], zoom) => Graphic::Voxel(
                graceful_load_segment_no_skin(specifier),
                Transform {
                    ori: Quaternion::rotation_x(rot_x * std::f32::consts::PI / 180.0)
                        .rotated_y(rot_y * std::f32::consts::PI / 180.0)
                        .rotated_z(rot_z * std::f32::consts::PI / 180.0),
                    offset: Vec3::from(*offset),
                    zoom: *zoom,
                    orth: true, // TODO: Is this what we want here? @Pfau
                    stretch: false,
                },
                SampleStrat::None,
            ),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ItemImagesSpec(HashMap<ItemKey, ImageSpec>);
impl assets::Asset for ItemImagesSpec {
    type Loader = assets::RonLoader;

    const EXTENSION: &'static str = "ron";
}

// TODO: when there are more images don't load them all into memory
pub struct ItemImgs {
    map: HashMap<ItemKey, Id>,
    manifest: AssetHandle<ItemImagesSpec>,
    not_found: Id,
}

impl ItemImgs {
    pub fn new(ui: &mut Ui, not_found: Id) -> Self {
        let manifest = ItemImagesSpec::load_expect("voxygen.item_image_manifest");
        let map = manifest
            .read()
            .0
            .iter()
            // TODO: what if multiple kinds map to the same image, it would be nice to use the same
            // image id for both, although this does interfere with the current hot-reloading
            // strategy
            .map(|(kind, spec)| (kind.clone(), ui.add_graphic(spec.create_graphic())))
            .collect();

        Self {
            map,
            manifest,
            not_found,
        }
    }

    pub fn img_ids(&self, item_key: ItemKey) -> Vec<Id> {
        if let ItemKey::TagExamples(keys) = item_key {
            return keys
                .iter()
                .filter_map(|k| self.map.get(k))
                .cloned()
                .collect();
        };
        match self.map.get(&item_key) {
            Some(id) => vec![*id],
            // There was no specification in the ron
            None => {
                log::warn!(
                    "missing specified image file (note: hot-reloading won't work here), item_key:{:?}",item_key,
                );
                Vec::new()
            },
        }
    }

    pub fn img_ids_or_not_found_img(&self, item_key: ItemKey) -> Vec<Id> {
        let mut ids = self.img_ids(item_key);
        if ids.is_empty() {
            ids.push(self.not_found)
        }
        ids
    }
}

// Copied from figure/load.rs
// TODO: remove code dup?
fn graceful_load_vox(specifier: &str) -> AssetHandle<DotVoxAsset> {
    let full_specifier: String = ["voxygen.", specifier].concat();
    match DotVoxAsset::load(full_specifier.as_str()) {
        Ok(dot_vox) => dot_vox,
        Err(_) => {
            log::error!("Could not load vox file for item images, {:?}", full_specifier);
            DotVoxAsset::load_expect("voxygen.voxel.not_found")
        },
    }
}
fn graceful_load_img(specifier: &str) -> Arc<DynamicImage> {
    let full_specifier: String = ["voxygen.", specifier].concat();
    let handle = match assets::Image::load(&full_specifier) {
        Ok(img) => img,
        Err(_) => {
            log::error!("Could not load image file for item images, {:?}", full_specifier);
            assets::Image::load_expect("voxygen.element.not_found")
        },
    };
    handle.read().to_image()
}

fn graceful_load_segment_no_skin(specifier: &str) -> Arc<Segment> {
    use common::figure::{mat_cell::MatCell, MatSegment};
    let mat_seg = MatSegment::from(&graceful_load_vox(specifier).read().0);
    let seg = mat_seg
        .map(|mat_cell| match mat_cell {
            MatCell::None => None,
            MatCell::Mat(_) => Some(MatCell::None),
            MatCell::Normal(data) => data.is_hollow().then(|| MatCell::None),
        })
        .to_segment(|_| Default::default());
    Arc::new(seg)
}
