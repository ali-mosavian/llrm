// excerpt of the pin d1106deb71 X86GenFastISel.inc (-gen-fast-isel)

Register fastEmit_ISD_ADD_MVT_i32_rr(MVT RetVT, Register Op0, Register Op1) {
  if (RetVT.SimpleTy != MVT::i32)
    return Register();
  if ((Subtarget->hasNDD())) {
    return fastEmitInst_rr(X86::ADD32rr_ND, &X86::GR32RegClass, Op0, Op1);
  }
  if ((!Subtarget->hasNDD())) {
    return fastEmitInst_rr(X86::ADD32rr, &X86::GR32RegClass, Op0, Op1);
  }
  return Register();
}

Register fastEmit_ISD_ADD_MVT_i32_ri(MVT RetVT, Register Op0, uint64_t imm1) {
  if (RetVT.SimpleTy != MVT::i32)
    return Register();
  if ((Subtarget->hasNDD())) {
    return fastEmitInst_ri(X86::ADD32ri_ND, &X86::GR32RegClass, Op0, imm1);
  }
  if ((!Subtarget->hasNDD())) {
    return fastEmitInst_ri(X86::ADD32ri, &X86::GR32RegClass, Op0, imm1);
  }
  return Register();
}

Register fastEmit_ISD_ADD_MVT_i64_ri_Predicate_i64immSExt32(MVT RetVT, Register Op0, uint64_t imm1) {
  if (RetVT.SimpleTy != MVT::i64)
    return Register();
  if ((Subtarget->hasNDD())) {
    return fastEmitInst_ri(X86::ADD64ri32_ND, &X86::GR64RegClass, Op0, imm1);
  }
  if ((!Subtarget->hasNDD())) {
    return fastEmitInst_ri(X86::ADD64ri32, &X86::GR64RegClass, Op0, imm1);
  }
  return Register();
}

Register fastEmit_ISD_ABS_MVT_v16i8_r(MVT RetVT, Register Op0) {
  if (RetVT.SimpleTy != MVT::v16i8)
    return Register();
  if ((Subtarget->hasBWI()) && (Subtarget->hasVLX())) {
    return fastEmitInst_r(X86::VPABSBZ128rr, &X86::VR128XRegClass, Op0);
  }
  if ((Subtarget->hasSSSE3() && !Subtarget->hasAVX())) {
    return fastEmitInst_r(X86::PABSBrr, &X86::VR128RegClass, Op0);
  }
  if ((Subtarget->hasAVX()) && (!Subtarget->hasVLX() || !Subtarget->hasBWI())) {
    return fastEmitInst_r(X86::VPABSBrr, &X86::VR128RegClass, Op0);
  }
  return Register();
}

Register fastEmit_ISD_ADD_rr(MVT VT, MVT RetVT, Register Op0, Register Op1) {
  switch (VT.SimpleTy) {
  case MVT::i8: return fastEmit_ISD_ADD_MVT_i8_rr(RetVT, Op0, Op1);
  case MVT::i16: return fastEmit_ISD_ADD_MVT_i16_rr(RetVT, Op0, Op1);
  case MVT::i32: return fastEmit_ISD_ADD_MVT_i32_rr(RetVT, Op0, Op1);
  case MVT::i64: return fastEmit_ISD_ADD_MVT_i64_rr(RetVT, Op0, Op1);
  case MVT::v16i8: return fastEmit_ISD_ADD_MVT_v16i8_rr(RetVT, Op0, Op1);
  case MVT::v32i8: return fastEmit_ISD_ADD_MVT_v32i8_rr(RetVT, Op0, Op1);
  case MVT::v64i8: return fastEmit_ISD_ADD_MVT_v64i8_rr(RetVT, Op0, Op1);
  case MVT::v8i16: return fastEmit_ISD_ADD_MVT_v8i16_rr(RetVT, Op0, Op1);
  case MVT::v16i16: return fastEmit_ISD_ADD_MVT_v16i16_rr(RetVT, Op0, Op1);
  case MVT::v32i16: return fastEmit_ISD_ADD_MVT_v32i16_rr(RetVT, Op0, Op1);
  case MVT::v4i32: return fastEmit_ISD_ADD_MVT_v4i32_rr(RetVT, Op0, Op1);
  case MVT::v8i32: return fastEmit_ISD_ADD_MVT_v8i32_rr(RetVT, Op0, Op1);
  case MVT::v16i32: return fastEmit_ISD_ADD_MVT_v16i32_rr(RetVT, Op0, Op1);
  case MVT::v2i64: return fastEmit_ISD_ADD_MVT_v2i64_rr(RetVT, Op0, Op1);
  case MVT::v4i64: return fastEmit_ISD_ADD_MVT_v4i64_rr(RetVT, Op0, Op1);
  case MVT::v8i64: return fastEmit_ISD_ADD_MVT_v8i64_rr(RetVT, Op0, Op1);
  default: return Register();
  }
}

