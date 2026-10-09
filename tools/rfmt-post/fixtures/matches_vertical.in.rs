fn f() {
    if matches!(
        operand,
        model::Operand::PlaceRef(_)
            | model::Operand::ArrayElement(_)
            | model::Operand::ProjectedPlace(_)
            | model::Operand::IndirectPlace(_)
    ) {
        g();
    }
}
