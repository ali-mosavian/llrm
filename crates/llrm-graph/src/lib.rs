//! Walks over a control-flow graph that any IR's blocks describe as
//! `loops::Node`s: the rich MIR's analyses and the machine phases share them.

pub mod loops;
