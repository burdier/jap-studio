pub mod component;
pub mod musical;
pub mod primitives;

pub use component::{Component, ComponentError, ControlEvent, NodeId};
pub use primitives::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};
