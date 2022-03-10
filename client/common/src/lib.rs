#![deny(unsafe_code)]
#![allow(incomplete_features)]
#![type_length_limit = "1664759"]
#![allow(clippy::option_map_unit_fn)]
#![deny(clippy::clone_on_ref_ptr)]
#![feature(
    associated_type_defaults,
    bool_to_option,
    fundamental,
    label_break_value,
    option_zip,
    trait_alias,
    type_alias_impl_trait,
    extend_one,
    arbitrary_enum_discriminant,
    generic_associated_types,
    arbitrary_self_types
)]
#![feature(hash_drain_filter)]

/// Re-exported crates

pub use uuid;

// modules

pub use common_assets as assets;
pub mod astar;

mod cached_spatial_grid;

pub mod calendar;

pub mod character;
pub mod clock;
pub mod cmd;
pub mod combat;
pub mod comp;
pub mod consts;
pub mod depot;

pub mod effect;
pub mod event;

pub mod explosion;

pub mod figure;

pub mod generation;
pub mod grid;
pub mod link;

pub mod lottery;

pub mod mounting;
pub mod npc;

pub mod outcome;
pub mod path;
pub mod ray;

pub mod recipe;

pub mod region;
pub mod resources;
pub mod rtsim;

pub mod skillset_builder;

pub mod slowjob;

pub mod spiral;

pub mod states;
pub mod store;

pub mod terrain;
pub mod time;
pub mod trade;
pub mod typed;
pub mod uid;
pub mod util;
pub mod vol;

pub mod volumes;


pub use cached_spatial_grid::CachedSpatialGrid;

pub use combat::{Damage, GroupTarget, Knockback, KnockbackDir};
pub use combat::{DamageKind, DamageSource};

pub use comp::inventory::loadout_builder::LoadoutBuilder;

pub use explosion::{Explosion, RadiusEffect};

pub use skillset_builder::SkillSetBuilder;
