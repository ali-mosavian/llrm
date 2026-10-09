fn f() {
    {
        let near = unit.operand_type(Operand::Value(value)).is_some_and(|ty| {
            matches!(
                unit.context.types.get(ty),
                Type::Pointer(space) if *space == unit.spaces().near
            )
        });
    }
}
