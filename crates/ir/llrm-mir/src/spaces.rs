//! What the address spaces are, by role: the numbers a target's description
//! gives them (`datalayout.toml`, `[spaces]`). No pass names a number; it asks
//! the target (`target::Machine::spaces`).

/// The address spaces of a target, by what a frontend calls them. Where two are
/// one space (`near` = `far`) the target is flat; one a target has no space for
/// is `None`, and a program using it is refused.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Spaces {
    pub near: u32,
    pub far: u32,
    /// Where the program's data is, for the passes that ask: its globals' and
    /// escaping locals' space.
    pub data: u32,
    /// Where the stack is.
    pub stack: u32,
    /// A selector alone.
    pub segment: Option<u32>,
    /// A far pointer whose offset carries into its selector.
    pub huge: Option<u32>,
    /// Memory no program object occupies: a device's registers or its frame
    /// buffer. A frontend states it of an access; an analysis reads it as
    /// "apart from every program object".
    pub fixed: Option<u32>,
    /// The most bytes a data segment holds: a larger object is cut into
    /// segments a huge pointer steps through. None where the target has no
    /// segments.
    pub segment_bytes: Option<u64>,
    /// Where real-mode memory is in a target with no selectors of its own: the address segment 0, offset 0 is at.
    /// A frontend whose program holds segment:offset pairs (BASIC's DEF SEG, VARSEG) means this memory by them,
    /// `base + segment * 16 + offset`. None where the target has selectors, or no such window.
    pub real_mode_base: Option<u64>,
}

impl Default for Spaces {
    fn default() -> Self {
        Self::FLAT
    }
}

impl Spaces {
    /// One space, none of the others: a target with no segments, and the one a
    /// pass is given when no target is named.
    pub const FLAT: Self =
        Self { near: 0, far: 0, data: 0, stack: 0, segment: None, huge: None, fixed: None, segment_bytes: None, real_mode_base: None };

    /// Whether `far` is the `near` space: the target is flat, and a far or huge
    /// pointer is a near one.
    #[inline]
    pub fn far_is_near(&self) -> bool {
        self.far == self.near
    }

    /// Whether `space` is the selector-alone space: never where the target has
    /// none.
    pub fn is_segment(
        &self,
        space: Option<u32>,
    ) -> bool {
        space.is_some() && space == self.segment
    }

    /// Whether `space` is the fixed-address space.
    pub fn is_fixed(
        &self,
        space: u32,
    ) -> bool {
        self.fixed == Some(space)
    }

    /// The selector-alone space, or why the target has none.
    pub fn segment_space(&self) -> Result<u32, String> {
        self.segment.ok_or_else(|| "this target has no selector address space".to_owned())
    }

    /// The huge-pointer space, or why the target has none.
    pub fn huge_space(&self) -> Result<u32, String> {
        self.huge.ok_or_else(|| "this target has no huge address space".to_owned())
    }

    /// The fixed-address space, or why the target has none.
    pub fn fixed_space(&self) -> Result<u32, String> {
        self.fixed.ok_or_else(|| "this target has no fixed address space".to_owned())
    }
}
