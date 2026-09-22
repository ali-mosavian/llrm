//! Ports of `qbopt/optimize`.

pub(crate) mod algebraic;
pub(crate) mod cfg;
pub(crate) mod edges;
pub(crate) mod indvars;
pub(crate) mod inline;
pub(crate) mod lcssa;
pub(crate) mod lcssamerges;
pub(crate) mod loadjoins;
pub(crate) mod loopclone;
pub(crate) mod loopsimplify;
pub(crate) mod pointeraccess;
pub(crate) mod profit;
pub(crate) mod promote;
pub(crate) mod rotate;
pub(crate) mod strength;
pub(crate) mod transform;
pub(crate) mod unswitch;
pub(crate) mod wholephis;
pub(crate) mod wholestores;
