//! The target nodes x86 gives the shared builder (`LoweringNodes`).

use llrm_codegen::backend::selectiondag::builder::LoweringNodes;
use super::x86isd as own;

use super::x86_lowering::x86isd as pin;

pub fn nodes() -> LoweringNodes {
    LoweringNodes {
        call: Some(own::CALL),
        ret: Some(own::RET_GLUE),
        string_move: Some(own::REP_MOVS),
        string_store: Some(own::REP_STOS),
        pop_float_result: Some(own::POP_FROM_X87_REG),
        tail_return: Some(own::TC_RETURN),
        far_tail_return: Some(own::LLRM_TC_RETURN),
        vector_save: Some(own::VASTART_SAVE_XMM_REGS),
        wide_divide: Some(own::LLRM_UDIV64BY32),
        call_flags: Some(own::LLRM_CALL_FLAGS),
        port_io: Some(own::LLRM_PORT),
        push_flags: Some(own::LLRM_PUSHF),
        copy128: Some(own::LLRM_COPY128),
        move128: Some(own::LLRM_MOVE128),
        zero128: Some(own::LLRM_ZERO128),
        window: Some(own::LLRM_WINDOW),
        ptrdiff: Some(own::LLRM_PTRDIFF),
        huge_add: Some(pin::HUGE_ADD),
        segment_of: Some(own::SEGMENT_OF),
        wrapper: Some(pin::WRAPPER),
        wrapper_rip: Some(pin::WRAPPER_RIP),
        far_load_es: Some(pin::LES),
        far_load_ds: Some(pin::LDS),
        far_selector_register: Some(super::x86_lowering::ES),
        far_call: Some(own::LLRM_FARCALL),
        flag_boolean: Some(pin::SETCC),
        asm: Some(own::LLRM_ASM),
        code: Some(own::LLRM_CODE),
    }
}
