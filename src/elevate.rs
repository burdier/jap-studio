pub mod actions;
pub mod catalog;
pub mod component;
pub mod devices;
pub mod explorer;
pub mod export;
pub mod musical;
pub mod paths;
pub mod primitives;
pub mod project;
pub mod scripting;

pub use component::{Component, ComponentError, ControlEvent, NodeId};
pub use primitives::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};
