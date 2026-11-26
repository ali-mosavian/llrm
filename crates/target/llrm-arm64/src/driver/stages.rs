//! The program as it comes to selection, built in stages: the mid-end, then
//! GlobalMerge. Each stage is kept by the settings it and the stages before it
//! read, so a trial that changes only a later stage takes the earlier ones as
//! they were built. A build that writes the stages takes none.

use llrm_mir::program::Program;
use llrm_transforms::trial::{self, Reuse, Site};

use super::{Level, lowered_before_selection, merged_globals, mid_end, program, with_aliases};
use crate::machine::Platform;

/// What a trial may tell the mid-end.
#[derive(Clone, PartialEq)]
pub(super) struct MidEnd {
    pub unrotated: bool,
    pub more_rotation: bool,
    /// 1 keeps the hinted calls, 2 those and the negative ones; bit 4 the
    /// calls at a loop site (INLINEHINT-LOOP-TRIAL).
    pub hinted: u8,
    /// The sites inlined whatever the size guard says.
    pub inlined: Vec<Site>,
}

impl MidEnd {
    fn told<T>(
        &self,
        run: impl FnOnce() -> T,
    ) -> T {
        let rotated = || match (self.unrotated, self.more_rotation) {
            (true, _) => trial::with_unrotated(run),
            (_, true) => trial::with_more_rotation(run),
            _ => run(),
        };
        let hinted = || match self.hinted & 3 {
            0 => rotated(),
            1 => trial::with_kept_hinted_calls(rotated),
            _ => trial::with_kept_hinted_calls(|| trial::with_kept_negative_calls(rotated)),
        };
        let looped = || if self.hinted & 4 != 0 { trial::with_kept_loop_calls(hinted) } else { hinted() };
        trial::with_inlined(&self.inlined, looped)
    }
}

/// One module's builds up to selection.
pub(super) struct Stages<'a> {
    text: &'a str,
    platform: Platform,
    level: Level,
    debug: bool,
    refuse_aliases: bool,
    after_mid_end: Reuse<MidEnd, Program>,
    after_merge: Reuse<(MidEnd, bool), Program>,
}

impl<'a> Stages<'a> {
    pub(super) fn new(
        text: &'a str,
        platform: Platform,
        level: Level,
        debug: bool,
        refuse_aliases: bool,
    ) -> Self {
        Stages {
            text,
            platform,
            level,
            debug,
            refuse_aliases,
            after_mid_end: Reuse::default(),
            after_merge: Reuse::default(),
        }
    }

    /// The program ready for selection; `dumped` builds every stage anew.
    pub(super) fn built(
        &self,
        settings: MidEnd,
        unmerged: bool,
        dumped: bool,
    ) -> Result<Program, String> {
        if self.level == Level::O0 {
            let mut program = self.parsed()?;
            lowered_before_selection(&mut program)?;
            return Ok(program);
        }
        if dumped {
            let optimized = self.optimized(&settings, true)?;
            return self.merged(optimized, unmerged);
        }
        self.after_merge.get_or((settings.clone(), unmerged), || {
            let optimized = self.after_mid_end.get_or(settings.clone(), || self.optimized(&settings, false))?;
            self.merged(optimized, unmerged)
        })
    }

    fn parsed(&self) -> Result<Program, String> {
        let mut program = program(self.text, self.platform)?;
        if self.refuse_aliases {
            // The text carries no alias: refuse it rather than drop the symbol.
            program.modules[0].refuse_aliases("arm64")?;
        }
        if self.debug {
            llrm_mir::dbgimport::types_and_globals(&mut program.modules[0]);
        }
        Ok(program)
    }

    fn optimized(
        &self,
        settings: &MidEnd,
        dumped: bool,
    ) -> Result<Program, String> {
        let mut program = self.parsed()?;
        settings.told(|| with_aliases(!self.refuse_aliases, || mid_end(&mut program, self.level, dumped)))?;
        Ok(program)
    }

    fn merged(
        &self,
        mut program: Program,
        unmerged: bool,
    ) -> Result<Program, String> {
        let mut run = || with_aliases(!self.refuse_aliases, || merged_globals(&mut program, self.level));
        if unmerged { crate::trial::without_merge(run) } else { run() }?;
        Ok(program)
    }
}
