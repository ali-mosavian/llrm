use qbfront::semantic::{compile_with_array_order, compile_with_options};
use qbfront::syntax::{Binary, ExitTarget, Expr, Literal, Procedure, Statement, TypeName};
use qbfront::{compile, parse, Dialect};

fn json_object_with<'a>(document: &'a str, field: &str) -> &'a str {
    let field_at = document
        .find(field)
        .unwrap_or_else(|| panic!("missing {field}: {document}"));
    let start = document[..field_at].rfind('{').expect("object start");
    let end = document[field_at..].find('}').expect("object end") + field_at + 1;
    &document[start..end]
}

#[test]
fn parses_long_array_and_whole_expression() {
    let module = parse(
        "dim shared samples(1 to 10) as long\nsamples(i) = samples(i) * 4 + bias&\n",
        Dialect::VbDos,
    )
    .unwrap();
    let Statement::Dim(declarations) = &module.statements[0] else {
        panic!()
    };
    assert_eq!(declarations[0].type_name, Some(TypeName::Long));
    assert_eq!(declarations[0].bounds.len(), 1);
    let Statement::Assign {
        value: Expr::Binary { op, .. },
        ..
    } = &module.statements[1]
    else {
        panic!()
    };
    assert_eq!(*op, Binary::Add);
}

#[test]
fn view_print_preserves_the_reset_and_bounded_runtime_forms() {
    // VBDOS emits B$VWPT(-1, -1) for the reset and B$VWPT(3, 22) for the
    // bounded form. Nibbles uses the reset before its opening CLS.
    let module = parse("view print\r\nview print 3 to 22\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }, Statement::Runtime { name: bounded, arguments: bounds, .. }]
            if name == "VIEW_PRINT" && arguments.is_empty() && bounded == "VIEW_PRINT" && bounds.len() == 2
    ));
    let hir = compile(&module, "view_print", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("B$VWPT").count(), 2);
    assert!(hir.contains("\"value\":-1"), "reset sentinels: {hir}");
    assert!(
        hir.contains("\"value\":3") && hir.contains("\"value\":22"),
        "bounds: {hir}"
    );
}

#[test]
fn console_input_retains_its_prompt_and_destination() {
    // Nibbles prompts into a dynamic string. Dropping the prompt expression
    // shifts it into the destination list and makes the literal assignable.
    let module = parse(
        "dim answer as string\r\ninput \"How many\"; answer\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert!(matches!(
        &module.statements[1],
        Statement::Input {
            prompt: Some(Expr::Literal(Literal::String(prompt), _)),
            suppress_question_mark: false,
            keep_cursor: false,
            destinations,
            ..
        } if prompt == "How many" && destinations.len() == 1
    ));

    let unprompted = parse("dim answer as string\r\ninput answer\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &unprompted.statements[1],
        Statement::Input { prompt: None, destinations, .. } if destinations.len() == 1
    ));
    let hir = compile(&module, "color", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$INPP\""), "{hir}");
    assert!(hir.contains("\"callee\":\"B$RDSD\""), "{hir}");
}

#[test]
fn input_declares_an_implicit_destination_before_building_its_type_table() {
    // Nibbles reads num$ and gamespeed$ without DIM. Microsoft BASIC's
    // implicit declaration must exist before B$INPP's type byte is chosen.
    let module = parse("input num$\r\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "implicit_input", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$INPP\""), "{hir}");
    assert!(hir.contains("\"callee\":\"B$RDSD\""), "{hir}");
}

#[test]
fn for_next_accepts_the_single_line_colon_form() {
    // Nibbles calibrates TIMER with an empty FOR/NEXT on one physical line.
    let module = parse("for i# = 1 to 1000: next i#\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::For { counter: Expr::Name(name, _), body, .. }]
            if name == "I#" && body.is_empty()
    ));
}

#[test]
fn play_uses_the_runtime_string_descriptor_contract() {
    // VBDOS PLAY.OBJ pushes one near string descriptor and calls B$SPLY,
    // whose far Pascal entry returns with RETF 2.
    let module = parse("play \"MBT160O1L8C\"\r\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "play", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$SPLY\""), "{hir}");
}

#[test]
fn color_retains_omitted_positional_arguments() {
    // Nibbles changes only the background with COLOR , background. The
    // omitted foreground must remain a positional fact, not shift arguments.
    let module = parse("color , 4\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }]
            if name == "COLOR" && arguments.len() == 2 && matches!(arguments[0], Expr::Omitted(_))
    ));
}

#[test]
fn palette_is_a_statement_inside_a_single_line_if() {
    // Gorillas probes EGA memory with `IF mode = 9 THEN PALETTE 4, 0`.
    let module = parse("if mode = 9 then palette 4, 0\r\n", Dialect::VbDos).unwrap();
    let Statement::If { then_branch, .. } = &module.statements[0] else {
        panic!("expected IF")
    };
    assert!(matches!(
        &then_branch[..],
        [Statement::Runtime { name, arguments, .. }] if name == "PALETTE" && arguments.len() == 2
    ));
}

#[test]
fn inline_def_fn_does_not_capture_the_following_function() {
    // Gorillas defines FnRan inline before the ordinary CalcDelay function.
    let module = parse(
        "def FnRan (x) = int(rnd(1) * x) + 1\r\n\
         function CalcDelay!\r\nCalcDelay! = 1\r\nend function\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert!(matches!(
        &module.procedures[..],
        [Procedure { name: first, body: first_body, .. }, Procedure { name: second, body: second_body, .. }]
            if first == "FNRAN" && first_body.len() == 1 && second == "CALCDELAY!" && second_body.len() == 1
    ));
}

#[test]
fn circle_retains_coordinates_radius_and_color() {
    // Gorillas draws an explosion with the four-operand CIRCLE form.
    let module = parse("circle (x, y), radius, color\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }] if name == "CIRCLE" && arguments.len() == 4
    ));
}

#[test]
fn circle_retains_omitted_angles_before_aspect() {
    let module = parse(
        "circle (x, y), radius, color, , , aspect\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }]
            if name == "CIRCLE" && arguments.len() == 7
                && matches!(arguments[4], Expr::Omitted(_))
                && matches!(arguments[5], Expr::Omitted(_))
    ));
}

#[test]
fn line_input_with_prompt_is_not_a_graphics_line() {
    // Gorillas uses the console form; dispatching only on the shared LINE
    // keyword silently turned it into a two-operand graphics statement.
    let module = parse("line input \"Name: \"; player$\r\n", Dialect::QuickBasic45).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::LineInput { file: None, prompt: Some(_), destination: Expr::Name(name, _), .. }]
            if name == "PLAYER$"
    ));
}

#[test]
fn line_retains_both_coordinates_color_and_box_fill() {
    // Gorillas clears the sun with LINE (x1,y1)-(x2,y2), color, BF.
    let module = parse("line (x1, y1)-(x2, y2), color, bf\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }]
            if name == "LINE" && arguments.len() == 6 && matches!(&arguments[5], Expr::Name(option, _) if option == "BF")
    ));
}

#[test]
fn paint_retains_coordinates_and_fill_color() {
    let module = parse("paint (x, y), color\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }] if name == "PAINT" && arguments.len() == 3
    ));
}

#[test]
fn pset_retains_coordinates_and_color() {
    let module = parse("pset (x, y), color\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }] if name == "PSET" && arguments.len() == 3
    ));
}

#[test]
fn graphics_put_retains_coordinates_array_and_raster_operation() {
    // Gorillas uses both PSET and XOR raster operations in single-line IF arms.
    let module = parse("put (x, y), banana, xor\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }]
            if name == "PUT" && arguments.len() == 4 && matches!(&arguments[3], Expr::Name(mode, _) if mode == "XOR")
    ));
}

#[test]
fn graphics_get_retains_rectangle_and_destination_array() {
    let module = parse("get (x1, y1)-(x2, y2), image\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::Runtime { name, arguments, .. }] if name == "GET" && arguments.len() == 5
    ));
}

#[test]
fn print_tab_retains_position_without_printing_it_as_a_number() {
    let module = parse("print name; tab(50); score\r\n", Dialect::VbDos).unwrap();
    let Statement::Print { items, .. } = &module.statements[0] else {
        panic!("expected PRINT")
    };
    assert!(
        matches!(&items[1].value, Expr::Apply { name, arguments, .. } if name == "TAB" && arguments.len() == 1)
    );
}

#[test]
fn locate_uses_vbdos_count_led_positional_arguments() {
    // Raw VBDOS LOCATE.OBJ emits LOCATE ,9 as 0,1,9,3 followed by B$LOCT.
    // Dropping the omitted row moves the column into the wrong slot.
    let module = parse("locate , 9\r\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "locate", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$LOCT\""), "{hir}");
    assert!(hir.contains(
        "\"operands\":[{\"tag\":\"constant\",\"type\":1,\"value\":0},{\"tag\":\"constant\",\"type\":1,\"value\":1},{\"tag\":\"constant\",\"type\":1,\"value\":9},{\"tag\":\"constant\",\"type\":1,\"value\":3}]"
    ), "{hir}");
}

#[test]
fn rnd_loads_the_single_returned_by_the_vbdos_runtime() {
    // VBDOS RND.OBJ pushes one R4 argument, calls B$RND1, then loads the
    // SINGLE through the near pointer returned in AX.
    let module = parse("dim value as single\r\nvalue = rnd(1)\r\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "rnd", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$RND1\""), "{hir}");
    assert!(hir.contains("\"op\":\"load\""), "{hir}");
    let call = hir.find("\"callee\":\"B$RND1\"").unwrap();
    assert!(
        hir[call..].starts_with("\"callee\":\"B$RND1\"")
            && hir[call..call + 180].contains("\"tag\":\"place\""),
        "{hir}"
    );
}

#[test]
fn floating_array_subscripts_are_converted_to_integer_indices() {
    // Nibbles first uses `a#` for timing and later indexes sammy(a). Microsoft
    // BASIC accepts the resulting DOUBLE subscript and converts it for lookup.
    let module = parse(
        "defint a-z\r\n\
         type Snake\r\ndirection as integer\r\nend type\r\n\
         declare sub moveSnake(s() as Snake)\r\n\
         sub moveSnake(s() as Snake)\r\nfor a# = 1 to 2\r\nnext a#\r\nfor a = 1 to 2\r\ns(a).direction = 1\r\nnext a\r\nend sub\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "defint_scope", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"convert\""), "{hir}");
}

#[test]
fn decoded_cp437_string_literals_are_encoded_back_to_dos_bytes() {
    // Nibbles' dialog border contains CP437 box drawing characters. Source
    // loading decodes them for parsing; emitted literal data must recover CD.
    let module = parse("print \"═\"\r\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "cp437", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("205"), "{hir}");
}

#[test]
fn bare_inkey_is_an_effectful_runtime_string_function() {
    // Nibbles uses the zero-argument INKEY$ spelling in WHILE conditions.
    let module = parse("while inkey$ = \"\": wend\r\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "inkey", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$INKY\""), "{hir}");
    assert!(hir.contains("\"callee\":\"B$SCMP\""), "{hir}");
}

#[test]
fn restore_label_uses_its_precomputed_read_data_offset() {
    // Raw VBDOS REST.OBJ pushes the key of the labeled DATA row and
    // calls B$RSTB, even when RESTORE precedes that label in source order.
    let module = parse(
        "restore laterData\r\nend\r\nfirstData: data 1\r\nlaterData: data 2\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "restore", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"$QB$RSTB:1\""), "{hir}");
}

#[test]
fn print_using_retains_format_separately_from_values() {
    // Nibbles' score line formats two values. The format descriptor is setup,
    // not a third value passed through the ordinary PRINT item path.
    let module = parse(
        "dim score as integer, lives as integer\r\n\
         print using \"#,###,#00  Lives: #\"; score; lives\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert!(matches!(
        &module.statements[1],
        Statement::Print { using: Some(Expr::Literal(Literal::String(format), _)), items, .. }
            if format == "#,###,#00  Lives: #" && items.len() == 2
    ));
    let hir = compile(&module, "print_using", Dialect::VbDos, "vbdos").unwrap();
    for callee in ["B$USNG", "B$PSI2", "B$PEI2"] {
        assert!(
            hir.contains(&format!("\"callee\":\"{callee}\"")),
            "{callee}: {hir}"
        );
    }
}

#[test]
fn while_wend_accepts_the_single_line_colon_form() {
    // Nibbles drains INKEY$ with compact WHILE condition: WEND loops.
    let module = parse("while inkey$ <> \"\": wend\r\n", Dialect::VbDos).unwrap();
    assert!(matches!(
        &module.statements[..],
        [Statement::While { body, .. }] if body.is_empty()
    ));
}

#[test]
fn parses_default_type_ranges_as_declarations_not_calls() {
    // DEFLNG previously reached semantic analysis as a call to a nonexistent
    // procedure, so ordinary Microsoft BASIC default typing could not compile.
    let module = parse("defint a-c\ndeflng l, x-z\n", Dialect::QuickBasic45).unwrap();
    assert!(matches!(
        &module.statements[0],
        Statement::DefType {
            type_name: TypeName::Integer,
            ranges,
            ..
        } if ranges == &[('A', 'C')]
    ));
    assert!(matches!(
        &module.statements[1],
        Statement::DefType {
            type_name: TypeName::Long,
            ranges,
            ..
        } if ranges == &[('L', 'L'), ('X', 'Z')]
    ));
}

#[test]
fn default_typing_applies_from_its_line_and_yields_to_suffix_and_as() {
    // BC 4.5's listing of `banana = 1: DEFINT B: banana = 2` stores BANANA!
    // then BANANA%: each name is typed where it appears, so `apple` either
    // side of DEFINT is two variables. PRINT LEN(apple) after it prints 2.
    let module = parse(
        "dim apple\ndefint a-a\ndim another, aLong&, appleDouble as double\nprint len(apple)\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "default_types", Dialect::QuickBasic45, "qb45").unwrap();
    for (name, extent, type_id) in [
        ("APPLE!", 4, 3),
        ("APPLE%", 2, 1),
        ("ANOTHER%", 2, 1),
        ("ALONG&", 4, 2),
        ("APPLEDOUBLE", 8, 4),
    ] {
        let place = json_object_with(&hir, &format!("\"name\":\"{name}\""));
        assert!(place.contains(&format!("\"extent\":{extent}")), "{place}");
        assert!(place.contains("\"storage\":\"module\""), "{place}");
        assert!(place.contains(&format!("\"type\":{type_id}")), "{place}");
    }
}

#[test]
fn module_for_temporaries_live_in_module_data_not_a_procedure_frame() {
    // qb-qrender MAIN reserved its complete 7.5 KiB BC_DATA extent again as
    // a B$ENRA frame because FOR's hidden end/step cells were based on the
    // module data cursor. That consumed nearly the complete linked stack and
    // made B$DDIM's string allocation overwrite SYS_PARSE_ARGS' local `cl`.
    let module = parse(
        "dim shared padding(0 to 1023) as integer\n\
         dim i as integer\n\
         for i = 1 to 2\n\
         next i\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "module_for", Dialect::VbDos, "vbdos").unwrap();
    for name in ["$forEnd", "$forStep"] {
        let start = hir
            .find(&format!("\"name\":\"{name}"))
            .expect("hidden FOR cell");
        let place = &hir[start..hir[start..].find('}').expect("place end") + start];
        assert!(place.contains("\"storage\":\"module\""), "{place}");
        assert!(!place.contains("\"offset\":-"), "{place}");
    }
}

#[test]
fn control_not_inverts_the_operand_truth_without_materializing_bitwise_not() {
    // qb-qrender's BSP walk must enter for node 0 and leave for node &h8000.
    // Materializing integer NOT cannot express that truth test: NOT &h8000 is
    // &h7fff, which is still true. In a control condition, NOT exchanges the
    // operand's true and false successors instead.
    let module = parse(
        "dim nodeIndex as integer\n\
         while not (nodeIndex and &h8000)\n\
         nodeIndex = nodeIndex - 1\n\
         wend\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "control_not", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"and\""));
    assert!(!hir.contains("\"op\":\"not\""));
    assert!(hir.contains(
        "\"kind\":\"branch\",\"operands\":[{\"tag\":\"value\",\"value\":2}],\"targets\":[4,3]"
    ));
}

#[test]
fn a_procedure_def_type_carries_into_the_procedures_after_it() {
    // QB 4.5, PDS 7.1 and VBDOS listings of this shape all store `beta = 3`
    // in `later` as an eight-byte DOUBLE: DEFtype is textual, not scoped to
    // the procedure it appears in.
    let module = parse(
        "defint a-a\nsub first\ndefdbl b-b\ndim apple, beta\nend sub\nsub later\ndim beta\nend sub\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "local_defaults", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(
        hir.matches("\"extent\":2,\"id\":1,\"name\":\"APPLE%\"")
            .count(),
        1,
        "{hir}"
    );
    assert_eq!(
        hir.matches("\"extent\":8,\"id\":2,\"name\":\"BETA#\"")
            .count(),
        1,
        "{hir}"
    );
    assert_eq!(
        hir.matches("\"extent\":8,\"id\":1,\"name\":\"BETA#\"")
            .count(),
        1,
        "{hir}"
    );
}

#[test]
fn bare_zero_argument_function_name_is_a_call_not_an_implicit_local() {
    // SYS_MEM_MARK printed zero because the declared external memAvail& was
    // silently materialized as a zero-initialized local LONG instead of called.
    let module = parse(
        "declare function answer& ()\nprint answer&\nfunction answer&\nanswer& = 42\nend function\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "bare_function", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"ANSWER&\""));
    assert!(!hir.contains("\"name\":\"ANSWER&\",\"offset\":0,\"storage\":\"module\""));
}

#[test]
fn timer_is_an_effectful_zero_argument_intrinsic_not_an_implicit_local() {
    // Fresh SYS_TIME_INIT repeatedly loaded one zeroed local and could never
    // leave `LOOP UNTIL TIMER <> t0`. QB/PDS/VBDOS all call B$TIMR, which
    // returns a pointer to a runtime-owned SINGLE.
    let module = parse(
        "dim started as single\nstarted = timer\ndo\nloop until timer <> started\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "timer_basic", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$TIMR\"").count(), 2);
    assert!(!hir.contains("\"name\":\"TIMER\",\"offset\""));
    assert!(hir.contains("\"op\":\"load\""));
}

#[test]
fn floating_function_has_the_hidden_microsoft_result_pointer() {
    // SYS_TICK_HZ returned on x87 and its callers read zero. BC passes a
    // hidden near destination after the source formals, so one SINGLE BYVAL
    // plus the result pointer is six callee-cleaned bytes.
    let module = parse(
        "declare function addHalf (byval value as single) as single\nprint addHalf(1.5)\nfunction addHalf (byval value as single) as single\naddHalf = value + .5\nend function\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "float_function", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"ADDHALF\",\"linkage\":\"external\",\"parameters\":[1,2]"));
    assert!(hir.contains("\"parameter_bytes\":6"), "{hir}");
    assert!(hir.contains("\"op\":\"address\""), "{hir}");
}

#[test]
fn parser_accepts_the_vbdos_source_superset_for_every_runtime_profile() {
    // Syntax is deliberately the VBDOS superset.  The selected profile
    // chooses ABI/runtime lowering, not which source spellings parse.
    assert!(parse("dim pos_x as long", Dialect::QuickBasic45).is_ok());
    assert!(parse("dim pos_x as long", Dialect::Pds71).is_ok());
    assert!(parse("dim pos_x as long", Dialect::VbDos).is_ok());
}

#[test]
fn parses_single_line_if_without_pcode() {
    let module = parse(
        "if count& > 0 then total& = total& + count& else goto done\n",
        Dialect::VbDos,
    )
    .unwrap();
    let Statement::If {
        then_branch,
        else_branch,
        ..
    } = &module.statements[0]
    else {
        panic!()
    };
    assert!(matches!(then_branch[0], Statement::Assign { .. }));
    assert!(matches!(else_branch[0], Statement::Goto(_, _)));
}

#[test]
fn colon_statements_stay_inside_a_single_line_if_arm() {
    // d_surf's LS_SELFTEST and SC_SELFTEST lost everything after their first
    // guard because EXIT FUNCTION after the colon escaped the IF arm and
    // became an unconditional procedure statement.
    let module = parse(
        "function classify (value as integer) as integer\n\
         if value < 0 then classify = -1: exit function\n\
         classify = 1\n\
         end function\n",
        Dialect::VbDos,
    )
    .unwrap();
    let body = &module.procedures[0].body;
    let Statement::If { then_branch, .. } = &body[0] else {
        panic!("expected single-line IF")
    };
    assert!(matches!(then_branch[0], Statement::Assign { .. }));
    assert!(matches!(
        then_branch[1],
        Statement::Exit(ExitTarget::Function, _)
    ));
    assert!(matches!(body[1], Statement::Assign { .. }));

    let hir = compile(&module, "colon_if", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains(
        "\"op\":\"store\",\"operands\":[{\"place\":1,\"tag\":\"place\"},{\"tag\":\"constant\",\"type\":1,\"value\":1}]"
    ));
}

#[test]
fn joins_continuations_and_parses_procedure_boundaries() {
    let module = parse(
        "declare function sum ( byval a as long, _\n byval b as long ) as long\nfunction sum (byval a as long, byval b as long) as long\nsum = a + b\nend function\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert_eq!(module.procedures.len(), 2);
    assert!(module.procedures[0].declaration);
    assert_eq!(module.procedures[1].body.len(), 1);
}

#[test]
fn unary_operator_advances_before_parsing_operand() {
    // in_main.bas overflowed the parser stack at a NOT expression because
    // the operator was recognized repeatedly without consuming its token.
    let module = parse(
        "sub poll()\nwhile not keyDown\nkeyDown = -keyDown\nwend\nend sub\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert_eq!(module.procedures.len(), 1);
}

#[test]
fn pretested_do_rechecks_its_condition() {
    // control.bas initially emitted the body backedge to itself, so a DO
    // WHILE that became false never left the loop.
    let module = parse(
        "dim x as integer\ndo while x < 3\nx = x + 1\nloop\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "loop", Dialect::VbDos, "vbdos").unwrap();
    let body = hir
        .split("\"id\":3,\"instructions\":")
        .nth(1)
        .unwrap()
        .split("{\"id\":4,\"instructions\":")
        .next()
        .unwrap();
    assert!(body.contains("\"kind\":\"jump\",\"operands\":[],\"targets\":[2]"));
}

#[test]
fn gorillas_not_and_loop_enters_when_both_terms_are_true() {
    // Gorillas drew no banana: Impact=0 and OnScreen=-1 skipped PlotShot's
    // body because a buried NOT exchanged the DO WHILE branch successors.
    let module = parse(
        "dim impact as integer\ndim onScreen as integer\nimpact = 0\nonScreen = -1\ndo while (not impact) and onScreen\nimpact = -1\nloop\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "gorillas_loop", Dialect::QuickBasic45, "qb45").unwrap();
    let test = hir
        .split("\"id\":2,\"instructions\":")
        .nth(1)
        .unwrap()
        .split("{\"id\":3,\"instructions\":")
        .next()
        .unwrap();
    assert!(
        test.contains("\"kind\":\"branch\"") && test.contains("\"targets\":[3,4]"),
        "PlotShot's true condition must enter the animation body: {test}"
    );
}

#[test]
fn postfix_subscript_can_follow_a_field_chain() {
    // qb-qrender uses VARSEG(g.wld.tex.ofs(0)); accepting calls only directly
    // after an identifier stopped at the inner `(` and rejected the module.
    let module = parse("x& = clng(varptr(g.wld.tex.ofs(0)))\n", Dialect::VbDos).unwrap();
    let Statement::Assign { value, .. } = &module.statements[0] else {
        panic!()
    };
    let Expr::Apply { arguments, .. } = value else {
        panic!()
    };
    let Expr::Apply { arguments, .. } = &arguments[0] else {
        panic!()
    };
    assert!(matches!(arguments[0], Expr::Index { .. }));
}

#[test]
fn array_parameter_access_is_generic_whole_pointer_hir() {
    // An array formal is a near pointer to its descriptor.  The descriptor's
    // selector and adjusted offset are loaded independently and concatenated
    // into the final far element pointer; it is not a frontend-specific MIR
    // operation or a single packed descriptor load.
    let module = parse(
        "sub fill(arr() as long)\narr(0) = 7\nend sub\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "array_parameter", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"address\":\"far\""));
    assert!(hir.contains("\"op\":\"concat\""));
    assert!(hir.contains("\"tag\":\"indirect\""));
}

#[test]
fn dynamic_array_redim_keeps_descriptor_identity() {
    // An empty DIM subscript list was once indistinguishable from a scalar,
    // so REDIM had no descriptor to initialize.
    let module = parse(
        "dim shared samples() as long\nredim samples(1 to 8) as long\n",
        Dialect::VbDos,
    )
    .unwrap();
    let Statement::Dim(items) = &module.statements[0] else {
        panic!()
    };
    assert!(items[0].array);
    let hir = compile(&module, "redim", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$RDIM\""));
    assert!(hir.to_ascii_lowercase().contains("samples$descriptor"));
}

#[test]
fn unspecified_rank_array_uses_the_bascom_eight_dimension_descriptor() {
    // Fresh SYS reserved 254 bytes for each one-dimensional shared array and
    // exceeded DGROUP at link time.  Raw VBDOS SYS.OBJ reserves 46 bytes:
    // the 14-byte AD header plus BASCOM's eight four-byte DM records.
    let module = parse(
        "dim shared samples() as long\nredim samples(1 to 8) as long\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "redim", Dialect::VbDos, "vbdos").unwrap();
    let descriptor = json_object_with(&hir, "\"name\":\"SAMPLES$descriptor\"");
    assert!(descriptor.contains("\"extent\":46"), "{descriptor}");
    assert!(descriptor.contains("\"offset\":0"), "{descriptor}");
    assert!(
        descriptor.contains("\"storage\":\"module\""),
        "{descriptor}"
    );
    assert!(hir.contains("\"kind\":\"opaque\",\"name\":\"SAMPLES descriptor\",\"rank\":0,\"signed\":null,\"width\":46"), "{hir}");
}

#[test]
fn select_case_builds_explicit_comparison_cfg() {
    let module = parse(
        "dim n as integer\nselect case n\ncase 1, 2\nn = 3\ncase else\nn = 4\nend select\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "select", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"eq\""));
    assert!(hir.contains("\"kind\":\"branch\""));
}

#[test]
fn select_case_accepts_same_line_arm_bodies() {
    // D_MDL keeps compact CASE labels and their assignments on one physical
    // line. A colon is a BASIC statement boundary here, just as a newline is.
    let module = parse(
        "dim n as integer\ndim result as integer\nselect case n\ncase 1: result = 10\ncase else: result = 20\nend select\n",
        Dialect::VbDos,
    )
    .unwrap();
    let Statement::Select {
        arms, otherwise, ..
    } = &module.statements[2]
    else {
        panic!("expected select")
    };
    assert_eq!(arms[0].1.len(), 1);
    assert_eq!(otherwise.len(), 1);
}

#[test]
fn bare_end_inside_structured_blocks_is_not_the_block_terminator() {
    // Most compatibility cases stop immediately on a failed checkpoint.
    // Consuming that END as the prefix of END IF/SELECT rejected the whole
    // source before the qbopt frontend could report its actual next gap.
    let module = parse(
        "dim n as integer\nif n then\nprint \"FAIL if\"\nend\nend if\nselect case n\ncase 1\nend\ncase else\nn = 2\nend select\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let Statement::If { then_branch, .. } = &module.statements[1] else {
        panic!()
    };
    assert!(matches!(
        &then_branch[1],
        Statement::Runtime { name, .. } if name == "END"
    ));
    let Statement::Select { arms, .. } = &module.statements[2] else {
        panic!()
    };
    assert!(matches!(
        &arms[0].1[0],
        Statement::Runtime { name, .. } if name == "END"
    ));
}

#[test]
fn bare_def_seg_restores_ds_through_the_audited_runtime_entry() {
    // Q45M12 reached its cleanup `DEF SEG` after PEEK/POKE. Requiring '='
    // rejected the source, while treating it like DEF SEG=0 would select the
    // zero segment rather than the program's DGROUP.
    let module = parse("def seg = 1234\ndef seg\n", Dialect::QuickBasic45).unwrap();
    assert!(matches!(
        &module.statements[0],
        Statement::DefSeg { value: Some(_), .. }
    ));
    assert!(matches!(
        &module.statements[1],
        Statement::DefSeg { value: None, .. }
    ));
    let hir = compile(&module, "defseg", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"callee\":\"B$DSG0\""));
}

#[test]
fn cls_is_runtime_and_poke_is_an_inline_segmented_store() {
    // Q45M12 previously became a call to an undeclared POKE procedure, and
    // the screen cases did the same for CLS.  The omitted CLS argument is
    // semantically distinct from CLS 0: QB45 represents it with -1.
    let module = parse("cls\ncls 2\npoke 100, 42\n", Dialect::QuickBasic45).unwrap();
    let hir = compile(&module, "screen_memory", Dialect::QuickBasic45, "qb45").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$SCLS\"").count(), 2);
    assert!(hir.contains("\"type\":1,\"value\":-1"));
    assert!(!hir.contains("\"callee\":\"B$POKE\""));
    assert!(hir.contains("\"op\":\"concat\""));
    assert!(hir.contains("\"op\":\"store\""));
    assert!(hir.contains("\"name\":\"b$seg\""));
    assert!(hir.contains("\"type\":8,\"value\":42"));
}

#[test]
fn swap_is_a_typed_inline_exchange() {
    // PL_MOVE uses SWAP inside a single-line IF. The recovered grammar emits
    // opStSwap; it must become value exchange HIR rather than a runtime call.
    let module = parse(
        "dim first as single\ndim second as single\nif first > second then swap first, second\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "swap", Dialect::VbDos, "vbdos").unwrap();
    assert!(!hir.contains("\"callee\":\"SWAP\""));
    assert!(hir.matches("\"op\":\"load\"").count() >= 2);
    assert!(hir.matches("\"op\":\"store\"").count() >= 2);
}

#[test]
fn numeric_line_labels_preserve_the_statement_on_the_same_source_line() {
    // Q45ER51's `100 quotient = ...` previously tried to parse the line
    // number as an expression statement and never reached ON ERROR/ERL.
    let module = parse(
        "dim value as integer\n100 value = 17\ngoto 100\non error goto 100\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    assert!(matches!(&module.statements[1], Statement::Label(name, _) if name == "100"));
    assert!(matches!(&module.statements[2], Statement::Assign { .. }));
    assert!(matches!(&module.statements[3], Statement::Goto(name, _) if name == "100"));
    assert!(matches!(
        &module.statements[4],
        Statement::OnError { label, .. } if label == "100"
    ));
}

#[test]
fn unsuffixed_real_precision_and_d_exponents_select_the_documented_type() {
    // Q45MT37/Q45TN74 rounded 16-digit anchors to SINGLE and failed at
    // 1e-12 tolerance. QB45 help makes >15 decimal digits and D exponents
    // DOUBLE, while a shorter unsuffixed decimal remains SINGLE.
    let module = parse(
        "dim precise as double\n\
         dim tolerance as single\n\
         dim exponent as double\n\
         precise = .7853981633974483\n\
         tolerance = .000000000001\n\
         exponent = 1D2\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let mut literals = module
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Assign {
                value: Expr::Literal(Literal::Real(_, type_name), _),
                ..
            } => Some(type_name),
            _ => None,
        });
    assert_eq!(literals.next(), Some(&TypeName::Double));
    assert_eq!(literals.next(), Some(&TypeName::Single));
    assert_eq!(literals.next(), Some(&TypeName::Double));
}

#[test]
fn resume_and_on_local_error_are_structured_control_transfers() {
    // RESUME NEXT used to parse NEXT as an expression, and ON LOCAL ERROR
    // was mistaken for the unrelated ON-dispatch statement.  Neither may be
    // represented as an ordinary user procedure call.
    let module = parse(
        "on local error goto caught\n100 error 53\ncaught:\nresume next\nresume 100\nresume\n",
        Dialect::VbDos,
    )
    .unwrap();
    assert!(matches!(
        &module.statements[0],
        Statement::OnError { label, local: true, .. } if label == "CAUGHT"
    ));
    assert!(matches!(
        &module.statements[4],
        Statement::Resume {
            target: qbfront::syntax::ResumeTarget::Next,
            ..
        }
    ));
    assert!(matches!(
        &module.statements[5],
        Statement::Resume { target: qbfront::syntax::ResumeTarget::Label(label), .. }
            if label == "100"
    ));
    assert!(matches!(
        &module.statements[6],
        Statement::Resume {
            target: qbfront::syntax::ResumeTarget::Current,
            ..
        }
    ));
}

#[test]
fn option_base_changes_only_omitted_array_lower_bounds() {
    // OPTION BASE was rejected syntactically, hiding every later array and
    // LBOUND/UBOUND obligation in Q45A05.
    let module = parse(
        "option base 1\ndim implicitBounds(3) as integer\ndim explicitBounds(0 to 3) as integer\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    assert!(matches!(&module.statements[0], Statement::OptionBase(1, _)));
    let hir = compile(&module, "optionbase", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"bounds\":[[1,3]]") && hir.contains("\"name\":\"IMPLICITBOUNDS[]\""));
    assert!(hir.contains("\"bounds\":[[0,3]]") && hir.contains("\"name\":\"EXPLICITBOUNDS[]\""));
}

#[test]
fn array_bounds_are_typed_descriptor_calls_not_array_element_syntax() {
    // LBOUND(values) was sent through ordinary Apply fallback and reported
    // that the LBOUND intrinsic itself was not an array.
    let module = parse(
        "dim values(2 to 4) as integer\ndim low as integer\ndim high as integer\nlow = lbound(values)\nhigh = ubound(values, 1)\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "bounds", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"callee\":\"B$LBND\""));
    assert!(hir.contains("\"callee\":\"B$UBND\""));
}

#[test]
fn array_order_is_an_explicit_compiler_option() {
    // VBDOS /R changes which subscript varies fastest. It is a compiler
    // switch, not a dialect or runtime-family property.
    let module = parse(
        "dim a(1 to 4, 2 to 6) as long\na(2, 2) = 1\n",
        Dialect::VbDos,
    )
    .unwrap();
    let column =
        compile_with_array_order(&module, "order", Dialect::VbDos, "vbdos", false).unwrap();
    let row = compile_with_array_order(&module, "order", Dialect::VbDos, "vbdos", true).unwrap();
    assert!(column.contains("\"array_order\":\"column-major\""));
    assert!(row.contains("\"array_order\":\"row-major\""));
    assert_ne!(column, row);
}

#[test]
fn huge_array_option_uses_the_measured_hary_contract() {
    // PDHUGE wrapped its 80,802-byte index at 64 KiB because /Ah never
    // reached semantic lowering and the descriptor selector never advanced.
    let module = parse(
        "'$dynamic\ndim a(0 to 200, -2 to 198) as integer\na(163, 3) = 222\n",
        Dialect::Pds71,
    )
    .unwrap();
    let ordinary = compile_with_options(
        &module,
        "ordinary",
        Dialect::Pds71,
        "pds71",
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    let huge = compile_with_options(
        &module,
        "huge",
        Dialect::Pds71,
        "pds71",
        false,
        true,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    assert!(!ordinary.contains("\"callee\":\"B$HARY\""));
    assert!(huge.contains("\"callee\":\"B$HARY\""));
    assert!(huge.contains("\"address\":\"huge\""));
    assert!(huge.contains("\"type\":1,\"value\":514"));
    assert_ne!(ordinary, huge);
}

#[test]
fn checked_array_option_routes_static_access_through_hary() {
    // PDRTC printed its no-error sentinel when /D was dropped and the
    // out-of-range static-array store was lowered as unchecked arithmetic.
    let module = parse("dim a(1) as integer\na(2) = 7\n", Dialect::Pds71).unwrap();
    let ordinary = compile_with_options(
        &module,
        "ordinary",
        Dialect::Pds71,
        "pds71",
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    let checked = compile_with_options(
        &module,
        "checked",
        Dialect::Pds71,
        "pds71",
        false,
        false,
        true,
        false,
        false,
        false,
    )
    .unwrap();
    assert!(!ordinary.contains("\"callee\":\"B$HARY\""));
    assert!(checked.contains("\"callee\":\"B$HARY\""));
    assert!(checked.contains("\"address\":\"huge\""));
}

#[test]
fn single_line_then_name_resolves_declared_sub_before_label() {
    // screen.bas says `IF redraw THEN scr_load_tick`. Treating every bare
    // name after THEN as a label rejected its declared zero-argument SUB.
    let module = parse(
        "declare sub tick()\ndim redraw as integer\nif redraw then tick\nsub tick()\nend sub\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "then_call", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"TICK\""));
}

#[test]
fn byref_numeric_expression_uses_an_addressable_temporary() {
    // screen.bas passes named numeric constants to ordinary BYREF formals;
    // they are expressions, not new implicit variables or illegal lvalues.
    let module = parse(
        "declare sub consume(x as integer)\nconst leftEdge = 10\nconsume leftEdge + 1\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "byref_temp", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"$arg"));
    assert!(hir.contains("\"op\":\"address\""));
}

#[test]
fn byref_string_literal_has_a_relocatable_descriptor() {
    // pl_move.bas passes an archive path literal to a STRING formal. The
    // literal is an SD (length + relocated near payload pointer), then SASS
    // materializes an owned descriptor because a user BYREF callee may
    // consume the expression temporary more than once.
    let module = parse(
        "declare sub consumeText(text as string)\nconsumeText \"assets.zip::clip.pag\"\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_literal", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"addend\":4,\"address\":\"near\",\"at\":2,\"target\":3"));
    assert!(hir.contains("\"name\":\"$string3$descriptor\""));
    assert!(hir.contains("\"name\":\"$stringArg"));
    assert!(hir.contains("\"callee\":\"B$SASS\""));
}

#[test]
fn globals_and_literals_have_distinct_backing_objects() {
    // A place id and a literal id previously both started at one. That made
    // the relocation name a different object depending on which table read
    // it. All module places now live in $data; literal descriptors own a
    // separate object.
    let module = parse(
        "declare sub consumeText(text as string)\ndim globalCount as long\nconsumeText \"A\"\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "data_objects", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"$data\",\"readonly\":false"));
    assert!(
        hir.contains("\"name\":\"GLOBALCOUNT\",\"offset\":0,\"storage\":\"module\",\"symbol\":1")
    );
    // $data and the statement table reserve identities 1 and 2.  The source
    // literal's payload is therefore object 3, independently of module
    // places which also begin at one.
    assert!(hir.contains("\"name\":\"$string3$payload\",\"readonly\":true"));
    assert!(hir.contains("\"target\":3"));
}

#[test]
fn seg_parameters_are_whole_far_pointers_in_hir() {
    // qrender's graphics declarations use VBDOS SEG formals. They are one
    // semantic pointer value, not two machine-register-flavoured words.
    let module = parse(
        "declare sub consume(seg item as long)\ndim item as long\nconsume item\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "seg_parameter", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"address\":\"far\""));
    assert!(hir.contains("\"kind\":\"pointer\",\"name\":\"far*long\""));
    assert!(hir.contains("\"op\":\"address\""));
    assert!(hir.contains("\"callee\":\"CONSUME\""));
}

#[test]
fn seg_any_accepts_any_addressable_actual_type() {
    // u3dMtrxLookAt and ugluCubicBez3D deliberately declare SEG AS ANY.
    // ANY erases the pointee type at that call boundary, not its address
    // width or the actual object's provenance.
    let module = parse(
        "declare sub consume(seg item as any)\ntype Point\nx as long\nend type\ndim point as Point\nconsume point\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "seg_any", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"CONSUME\""));
    assert!(hir.contains("\"name\":\"far*POINT\""));
}

#[test]
fn fixed_string_assignment_uses_audited_assn_with_far_addresses() {
    // d_surf.bas assigns literals into fixed fields. B$ASSN receives source
    // data, source width, destination data, destination width; the string
    // operation itself does not become a MIR primitive.
    let module = parse("dim label as string * 8\nlabel = \"sky\"\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "fixed_string", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$ASSN\""));
    assert!(hir.contains("\"name\":\"$string3$payload\""));
    assert!(hir.contains("\"value\":3"));
    assert!(hir.contains("\"value\":8"));
}

#[test]
fn varseg_and_varptr_project_one_whole_pointer() {
    // h_frame.bas rebuilds a packed far pointer from these two language
    // intrinsics. HIR names the 16:16 projections; MIR chooses the existing
    // shift/copy forms without exposing registers to the frontend.
    let module = parse(
        "dim item as long\ndim segPart as integer\ndim offPart as integer\nsegPart = varseg(item)\noffPart = varptr(item)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "pointer_parts", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"pointer_segment\""));
    assert!(hir.contains("\"op\":\"pointer_offset\""));
}

#[test]
fn floating_literal_is_typed_readonly_data_before_mir() {
    // ent.bas first reached MIR with a host-number HIR constant, for which
    // there is no x87 immediate form. The source precision is rounded once
    // into explicit target bytes and loaded with floating semantics.
    let module = parse("dim value as single\nvalue = .1\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "float_literal", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"bytes\":[205,204,204,61]"));
    assert!(hir.contains("\"name\":\"$float3\""));
    assert!(hir.contains("\"op\":\"load\""));
}

#[test]
fn identical_floating_literals_share_the_module_constant_pool() {
    // QGL's repeated coordinate constants inflated BC_CN by 3440 bytes and
    // made the VBDOS runtime report Out of string space before MAIN began.
    let module = parse(
        "dim first as single\ndim second as single\nfirst = .5\nsecond = .5\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "pooled_float", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"$float3\""));
    assert!(!hir.contains("\"name\":\"$float4\""));
}

#[test]
fn procedure_locals_are_below_bp_and_parameters_start_above_the_return_address() {
    // The first source adapter used offset zero for a BYVAL copy, which
    // would overwrite saved BP. ABI parameters are values loaded from BP+6;
    // their addressable source copies and ordinary locals live below BP.
    let module = parse(
        "sub sample(byval inputValue as long)\ndim localValue as integer\nlocalValue = cint(inputValue)\nend sub\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "frame", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"INPUTVALUE\",\"offset\":-4,\"storage\":\"local\""));
    assert!(hir.contains("\"name\":\"LOCALVALUE\",\"offset\":-6,\"storage\":\"local\""));
    assert!(hir.contains("\"parameter_bytes\":4"));
}

#[test]
fn static_procedure_arrays_use_persistent_storage_without_exit_cleanup() {
    // Q45M28 printed "FAIL memorymodel lifetime": NEXTVALUE's COUNTS array
    // was allocated in the call frame and erased at every function return,
    // so two calls both returned 1 instead of returning 1 then 2.
    let module = parse(
        "declare function nextValue () as integer\n\
         function nextValue () as integer static\n\
         dim counts(0 to 0) as integer\n\
         counts(0) = counts(0) + 1\n\
         nextValue = counts(0)\n\
         end function\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "static_array", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"name\":\"COUNTS\",\"offset\":0,\"storage\":\"static\""));
    assert!(!hir.contains("\"callee\":\"B$ERAS\""));
}

#[test]
fn nonstatic_procedure_arrays_are_dynamic_despite_module_static_default() {
    // PDS and VBDOS document this exception explicitly. Treating COUNTS as
    // a fixed frame array gave it a static descriptor but then called B$ERAS
    // on that descriptor at exit, combining two incompatible representations.
    let module = parse(
        "' $STATIC\n\
         sub worker\n\
         dim counts(0 to 0) as integer\n\
         counts(0) = 1\n\
         end sub\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "dynamic_local", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"callee\":\"B$DDIM\""));
    assert!(hir.contains("\"callee\":\"B$ERAS\""));
    assert!(
        hir.contains("\"name\":\"COUNTS$descriptor\"") && hir.contains("\"storage\":\"local\"")
    );
}

#[test]
fn rem_array_metacommands_match_apostrophe_metacommands() {
    // Gorillas switches back with REM $STATIC after an apostrophe $DYNAMIC.
    // Discarding every character after REM left the second form semantically
    // inert even though Microsoft documents the two spellings as equivalent.
    let rem = parse(
        "REM $DYNAMIC\n\
         dim values(1 to 2) as integer\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let apostrophe = parse(
        "' $DYNAMIC\n\
         dim values(1 to 2) as integer\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    assert!(matches!(
        &rem.statements[1],
        Statement::Dim(declarations) if declarations[0].dynamic
    ));
    assert!(matches!(
        &apostrophe.statements[1],
        Statement::Dim(declarations) if declarations[0].dynamic
    ));
    let hir = compile(&rem, "rem_dynamic", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"callee\":\"B$DDIM\""));
}

#[test]
fn automatic_string_locals_are_deleted_before_runtime_frame_exit() {
    // Gorillas printed GI8P and then stopped with "String space corrupt":
    // Char$ survived UCASE$/SCMP but the procedure reached B$EXSA without
    // the B$STDL emitted by QB for every owned local descriptor.
    let module = parse(
        "sub probe\n\
         charValue$ = \"P\"\n\
         if ucase$(charValue$) = \"V\" then print \"BAD\"\n\
         end sub\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "string_cleanup", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"callee\":\"B$STDL\""));
}

#[test]
fn static_statement_declares_persistent_procedure_storage() {
    // Q45S15 stopped in the generated parser at NtACTIONidStatic. Once
    // parsed, QCOUNT's tally must be a data object, not a BP-relative local.
    let module = parse(
        "function qCount (qInput as integer)\n\
         static qTally as integer\n\
         qTally = qTally + 1\n\
         qCount = qTally + qInput\n\
         end function\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    assert!(matches!(module.procedures[0].body[0], Statement::Static(_)));
    let hir = compile(&module, "static_statement", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"name\":\"QTALLY\",\"offset\":0,\"storage\":\"static\""));
}

#[test]
fn sin_cos_and_tan_are_inline_float_hir() {
    // Math must remain visible computation and must not survive as a BASIC
    // runtime call. TAN is the reusable sin/cos/div identity.
    let module = parse(
        "dim x as single\ndim y as double\nx = sin(x)\ny = cos(y)\nx = tan(x)\ny = atn(y)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "trig", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"fsin\""));
    assert!(hir.contains("\"op\":\"fcos\""));
    assert!(hir.contains("\"op\":\"fdiv\""));
    assert!(hir.contains("\"op\":\"fatan\""));
    assert!(!hir.contains("B$SIN"));
    assert!(!hir.contains("B$COS"));
    assert!(!hir.contains("B$TAN"));
    assert!(!hir.contains("B$ATN"));
}

#[test]
fn int_preserves_integral_values_and_expands_float_floor() {
    // d_surf uses INT8 twice in one expression. INT is floor, not C-style
    // truncation, so the inline expansion includes a comparison/correction.
    let module = parse(
        "dim i as integer\ndim x as single\ndim y as double\ni = int(i)\nx = int(x)\ny = int(y)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "int", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"convert\""));
    assert!(hir.contains("\"op\":\"lt\""));
    assert!(hir.contains("\"op\":\"fadd\""));
    assert!(!hir.contains("B$INT"));
}

#[test]
fn power_and_integral_abs_are_visible_inline_hir() {
    // qb-qrender uses 2^i pervasively. It must be mathematical HIR, not the
    // legacy exponentiation runtime call; integral ABS is ordinary bit math.
    let module = parse(
        "dim i as integer\ndim x as single\nx = 2 ^ i\nx = 3 ^ i\ni = abs(i)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "power", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"op\":\"flog2\"").count(), 1);
    assert!(hir.contains("\"op\":\"fmul\""));
    assert_eq!(hir.matches("\"op\":\"fexp2\"").count(), 2);
    assert!(hir.contains("\"op\":\"sar\""));
    assert!(hir.contains("\"op\":\"xor\""));
    assert!(!hir.contains("B$EXP"));
}

#[test]
fn log_exp_and_fix_expand_to_visible_math_without_runtime_calls() {
    let module = parse(
        "dim x as double\nx = log(x)\nx = exp(x)\nx = fix(x)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "more_math", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"flog2\""));
    assert!(hir.contains("\"op\":\"fexp2\""));
    assert!(hir.contains("\"op\":\"fsub\""));
    assert!(!hir.contains("B$LOG"));
    assert!(!hir.contains("B$EXP"));
    assert!(!hir.contains("B$FIX"));
}

#[test]
fn integral_sqrt_and_sign_are_inline_typed_math() {
    let module = parse(
        "dim i as integer\ndim x as single\nx = sqr(i)\ni = sgn(i)\nx = sgn(x)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "sqrt_sign", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"fsqrt\""));
    assert!(hir.contains("\"op\":\"sub\""));
    assert!(hir.contains("\"op\":\"fsub\""));
    assert!(!hir.contains("B$SQR"));
    assert!(!hir.contains("B$SGN"));
}

#[test]
fn floating_conditions_compare_explicitly_with_zero() {
    // r_bsp branches on a SINGLE expression. QB accepts numeric conditions;
    // rejecting every non-integral condition prevented the whole module.
    let module = parse(
        "dim x as single\nif x then x = 1\ndo while x\nx = x - 1\nloop\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "float_truth", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.matches("\"op\":\"ne\"").count() >= 2);
    assert!(hir.contains("\"kind\":\"branch\""));
}

#[test]
fn def_seg_and_peek_form_a_typed_far_byte_load() {
    // r_bsp reads compressed PVS bytes using DEF SEG + PEEK. Preserve the
    // segment state as a source place and build an explicit 16:16 pointer.
    let module = parse(
        "dim segValue as integer\ndim offsetValue as long\ndim answer as integer\ndef seg = segValue\nanswer = peek(offsetValue)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "peek", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"concat\""));
    assert!(hir.contains("\"address\":\"far\""));
    assert!(hir.contains("\"name\":\"$byte\""));
    assert!(!hir.contains("B$PEEK"));
}

#[test]
fn erase_passes_each_dynamic_array_descriptor_to_the_runtime() {
    // r_bsp releases its REDIMed Leaf array. ERASE is not math: retain the
    // audited B$ERAS heap effect and pass the descriptor, not an element.
    let module = parse(
        "dim values() as integer\nredim values(7) as integer\nerase values\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "erase", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("B$RDIM"));
    assert!(hir.contains("B$ERAS"));
    assert!(hir.contains("descriptor"));
}

#[test]
fn any_array_formal_accepts_a_typed_dynamic_array() {
    // uglArrMap declares a() AS ANY specifically so a UDT array descriptor
    // can be rebound. Requiring the element type to equal ANY rejected the
    // operation the declaration exists to permit.
    let module = parse(
        "declare function map&(a() as any)\ntype item\nvalue as integer\nend type\ndim values() as item\nredim values(7) as item\ndim p as long\np = map&(values())\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "any_array", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.matches("\"op\":\"call\"").count() >= 2);
    assert!(hir.contains("descriptor"));
}

#[test]
fn any_array_declaration_may_be_refined_by_the_definition() {
    // Nibbles publishes EraseSnake with AS ANY arrays, then defines the body
    // with its concrete UDT element types. The public signature stays erased
    // while the body needs the refined fields.
    let module = parse(
        "declare sub refine(values() as any)\r\n\
         type Item\r\nvalue as integer\r\nend type\r\n\
         sub refine(values() as Item)\r\nvalues(0).value = 1\r\nend sub\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "any_definition", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"REFINE\""), "{hir}");
    assert!(hir.contains("\"name\":\"ITEM\""), "{hir}");
}

#[test]
fn isolated_direct_module_gosub_is_inlined() {
    // Nibbles keeps small module-level helpers after END and enters them with
    // GOSUB. A single direct use stays in the module frame and is not emitted
    // as an unresolved runtime symbol or a new far procedure.
    let module = parse(
        "dim total as integer\r\ngosub addTen\r\nend\r\naddTen:\r\ntotal = total + 10\r\nreturn\r\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "gosub", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"add\""), "{hir}");
    assert!(!hir.contains("\"callee\":\"GOSUB\""), "{hir}");
}

#[test]
fn asc_is_an_audited_runtime_call_over_a_string_descriptor() {
    let module = parse("dim code as integer\ncode = asc(\"A\")\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "asc", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FASC\""));
    assert!(hir.contains("\"name\":\"$string3$descriptor\""));
}

#[test]
fn string_runtime_results_are_near_descriptor_addresses() {
    // mod_tex.bas's suffix assignment is LDFS -> RTRM -> FMID -> SASS in
    // VBDOS output.  Each function returns a descriptor address in AX, not a
    // four-byte STRING value in AX:DX; omitted MID$ length is 7fffh.
    let module = parse(
        "type Texture\nname as string * 16\nend type\ndim texture as Texture\ndim suffix as string\ndim parts(0 to 3) as string\ndim oneChar as string * 1\ndim i as integer\nsuffix = mid$(rtrim$(texture.name), 3)\nparts(i) = mid$(suffix, i, 1)\noneChar = mid$(suffix, i, 1)\nsuffix = chr$(65)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_results", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"kind\":\"pointer\",\"name\":\"near*string\""));
    for callee in ["B$LDFS", "B$RTRM", "B$FMID", "B$SASS", "B$FCHR"] {
        assert!(hir.contains(&format!("\"callee\":\"{callee}\"")));
    }
    assert!(hir.contains("\"value\":32767"));
    assert!(hir.contains("\"name\":\"far*string\""));
    assert!(hir.contains("\"op\":\"ptr_offset\""));
}

#[test]
fn string_comparison_is_a_relation_over_scmp_flags() {
    // common.bas compares a fixed one-byte local with a fixed-string array
    // element. B$SCMP returns flags, so HIR must retain the relation rather
    // than claim that an INTEGER arrived in AX.
    let module = parse(
        "dim char as string * 1\ndim token(0 to 3) as string * 1\ndim i as integer\nif char = token(i) then i = i + 1\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_compare", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"string_eq\""));
    assert!(hir.contains("\"callee\":\"B$SCMP\""));
    assert!(hir.contains("\"kind\":\"branch\""));
}

#[test]
fn runtime_string_temporary_can_feed_a_string_formal() {
    // d_surf.bas calls LS_LCHAR(MID$(...)); the temporary result is already
    // the descriptor address the default BYREF STRING formal expects.
    let module = parse(
        "declare function consume%(text as string)\ndim source as string\ndim answer as integer\nanswer = consume%(mid$(source, 2, 1))\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_argument", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FMID\""));
    assert!(hir.contains("\"callee\":\"CONSUME%\""));
    assert!(hir.contains("\"name\":\"near*string\""));
}

#[test]
fn string_concatenation_is_a_descriptor_chain() {
    // common.bas repeatedly appends fixed and dynamic strings. B$SCAT takes
    // two descriptor addresses and returns another in AX; nested additions
    // therefore form a left-to-right typed call chain.
    let module = parse(
        "dim text as string\ndim oneChar as string * 1\ntext = text + oneChar + \"!\"\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_concat", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$SCAT\"").count(), 2);
    assert!(hir.contains("\"callee\":\"B$LDFS\""));
    assert!(hir.contains("\"callee\":\"B$SASS\""));
}

#[test]
fn simple_file_lifecycle_keeps_measured_runtime_operands() {
    // VBDOS listings spell INPUT/OUTPUT/BINARY as 1/2/20h and pass filename,
    // file number, -1, mode to B$OPEN. CLOSE appends the file-count word.
    let module = parse(
        "dim f as integer\ndim done as integer\nf = freefile\nopen \"x\" for input as #f\ndone = eof(f)\nclose #f\nopen \"y\" for output as #f\nclose #f\nopen \"z\" for binary as #f\nclose #f\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "files", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FREF\""));
    assert_eq!(hir.matches("\"callee\":\"B$OPEN\"").count(), 3);
    assert_eq!(hir.matches("\"callee\":\"B$CLOS\"").count(), 3);
    assert!(hir.contains("\"callee\":\"B$FEOF\""));
    for mode in [1, 2, 32] {
        assert!(hir.contains(&format!("\"value\":{mode}")));
    }
}

#[test]
fn fre_dispatches_numeric_and_string_selectors_to_their_runtime_entries() {
    // SYS_MEM_MARK uses both forms. VBDOS emits B$FRI2(-1) for numeric
    // heap selection and B$FRSD(&descriptor) for a string selector; treating
    // the latter as a numeric expression rejects valid source before HIR.
    let module = parse(
        "dim nearFree as long\ndim stringFree as long\nnearFree = fre(-1)\nstringFree = fre(\"\")\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "fre_forms", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FRI2\""));
    assert!(hir.contains("\"callee\":\"B$FRSD\""));
}

#[test]
fn line_input_keeps_disk_selection_and_destination_descriptor() {
    // common.bas emits B$DSKI(file), then B$LNIN(0, DS:&dynamic-string,
    // 0, 1). The latter is ten bytes of arguments and returns with RETF 10.
    let module = parse(
        "dim f as integer\ndim text as string\nline input #f, text\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "line_input", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$DSKI\""));
    assert!(hir.contains("\"callee\":\"B$LNIN\""));
    assert!(hir.contains("\"name\":\"far*string\""));
}

#[test]
fn string_select_case_is_scmp_control_flow() {
    // common.bas dispatches configuration keys with SELECT CASE over a
    // dynamic-string array element. Each arm is an ordinary SCMP equality
    // and branch; the selector descriptor itself is formed only once.
    let module = parse(
        "dim token(0 to 3) as string\ndim answer as integer\nselect case token(0)\ncase \"x\", \"y\"\nanswer = 1\ncase else\nanswer = 2\nend select\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_select", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"op\":\"string_eq\"").count(), 2);
    assert_eq!(hir.matches("\"callee\":\"B$SCMP\"").count(), 2);
    assert!(hir.matches("\"kind\":\"branch\"").count() >= 2);
}

#[test]
fn val_loads_the_double_dac_returned_by_fval() {
    // common.bas's VBDOS listing pushes a descriptor, calls B$FVAL, moves AX
    // to an address register, and loads a qword before numeric conversion.
    let module = parse(
        "dim text as string\ndim i as integer\ndim x as single\ni = val(text)\nx = val(text)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "val", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$FVAL\"").count(), 2);
    assert!(hir.contains("\"name\":\"near*double\""));
    assert!(hir.contains("\"indirect\""));
    assert!(hir.contains("\"op\":\"convert\""));
}

#[test]
fn source_string_function_result_is_a_descriptor_address() {
    // common.bas calls VAL(COM_ARG(...)); COM_ARG returns its dynamic STRING
    // descriptor address in AX, which feeds FVAL directly without a copy.
    let module = parse(
        "declare function getText(items() as string, count as integer) as string\ndim items(0 to 3) as string\ndim count as integer\ndim i as integer\ndim fixed as string * 8\ni = val(getText(items(), count))\nfixed = getText(items(), count)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_function", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"GETTEXT\""));
    assert!(hir.contains("\"callee\":\"B$FVAL\""));
    assert!(hir.contains("\"name\":\"near*string\""));
}

#[test]
fn str_selects_the_runtime_by_numeric_storage_type() {
    let module = parse(
        "dim i as integer\ndim l as long\ndim s as single\ndim d as double\ndim text as string\ntext = str$(i) + str$(l) + str$(s) + str$(d)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "str", Dialect::VbDos, "vbdos").unwrap();
    for callee in ["B$STI2", "B$STI4", "B$STR4", "B$STR8"] {
        assert!(hir.contains(&format!("\"callee\":\"{callee}\"")));
    }
    assert_eq!(hir.matches("\"callee\":\"B$SCAT\"").count(), 3);
}

#[test]
fn dynamic_string_array_uses_its_measured_near_descriptor_offset() {
    // COM_TOKENIZE's array descriptor carries a dword data pointer, but VBDOS
    // extracts its offset and passes that near descriptor address to SASS.
    // The operation is generic `ptr_offset` and its result is near*string;
    // no QB array-layout operation crosses the syntax/HIR boundary.
    let module = parse(
        "dim items() as string\ndim i as integer\ndim source as string\nredim items(0 to 3) as string\nitems(i) = source\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "far_string", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$SASS\""));
    assert!(hir.contains("\"op\":\"ptr_offset\""));
    assert!(hir.contains("\"name\":\"near*string\""));
}

#[test]
fn integral_operators_explicitly_round_floating_operands() {
    // d_surf and screen divide by 2^m with integer division. Power is
    // floating in QB; the following back-conversion is part of \ semantics.
    let module = parse(
        "dim extent as integer\ndim mip as integer\ndim scaled as integer\nscaled = extent \\ (2 ^ mip)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "integer_divide", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"fexp2\""));
    assert!(hir.contains("\"op\":\"convert\""));
    assert!(hir.contains("\"op\":\"div\""));
}

#[test]
fn constant_integral_powers_inline_variable_bases() {
    // QGL squares three signed coordinate differences. The base is not known
    // positive, so log2/exp2 is both needlessly expensive and domain-wrong.
    let module = parse(
        "dim x as single\ndim squared as single\nsquared = x ^ 2\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "square", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"fmul\""));
    assert!(!hir.contains("\"op\":\"flog2\""));
    assert!(!hir.contains("\"op\":\"fexp2\""));
}

#[test]
fn redim_can_declare_a_dynamic_array_under_option_explicit() {
    // screen.bas declares its temporary palette directly with REDIM; QB does
    // not require a preceding DIM for a dynamic array declaration.
    let module = parse(
        "option explicit\ntype Pixel\nr as integer\nend type\nredim palette(255) as Pixel\nerase palette\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "redim_declaration", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$RDIM\""));
    assert!(hir.contains("\"callee\":\"B$ERAS\""));
    assert!(hir.contains("PALETTE$descriptor"));
}

#[test]
fn mid_assignment_retains_its_full_runtime_shape() {
    let module = parse(
        "dim row as string\ndim x as integer\nmid$(row, x + 1, 1) = chr$(65)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "mid_assignment", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FCHR\""));
    assert!(hir.contains("\"callee\":\"B$SMID\""));
    assert!(hir.contains("\"name\":\"far*string\""));
}

#[test]
fn fixed_string_udt_fields_participate_in_concatenation() {
    // screen.bas concatenates the one-byte red/green/blue fields of tRGB.
    let module = parse(
        "type Pixel\nred as string * 1\ngreen as string * 1\nend type\ndim pixel as Pixel\ndim text as string\ntext = pixel.red + pixel.green\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "field_concat", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$LDFS\"").count(), 2);
    assert!(hir.contains("\"callee\":\"B$SCAT\""));
}

#[test]
fn mki_and_mkl_are_typed_binary_string_conversions() {
    let module = parse(
        "dim i as integer\ndim l as long\ndim text as string\ntext = mki$(i) + mkl$(l)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "mk_strings", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FMKI\""));
    assert!(hir.contains("\"callee\":\"B$FMKL\""));
    assert!(hir.contains("\"callee\":\"B$SCAT\""));
}

#[test]
fn unpositioned_get_put_keep_far_record_pointer_and_width() {
    let module = parse(
        "type Header\nsize as long\nend type\ndim f as integer\ndim header as Header\ndim text as string\nget #f, , header\nput #f, , text\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "record_io", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$GET3\""));
    assert!(hir.contains("\"callee\":\"B$PUT3\""));
    assert!(hir.contains("\"name\":\"far*HEADER\""));
}

#[test]
fn seek_and_positioned_transfer_keep_long_record_numbers() {
    let module = parse(
        "type Header\nsize as long\nend type\ndim f as integer\ndim recordNumber as long\ndim header as Header\nseek #f, recordNumber\nget #f, recordNumber, header\nput #f, recordNumber, header\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "positioned_io", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$SSEK\""));
    assert!(hir.contains("\"callee\":\"B$GET4\""));
    assert!(hir.contains("\"callee\":\"B$PUT4\""));
}

#[test]
fn left_and_both_string_forms_produce_descriptors() {
    let module = parse(
        "dim text as string\ndim count as integer\ntext = left$(text, 1) + string$(count, 0) + string$(count, \"x\")\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "string_builders", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$LEFT\""));
    assert!(hir.contains("\"callee\":\"B$STRI\""));
    assert!(hir.contains("\"callee\":\"B$STRS\""));
    assert_eq!(hir.matches("\"callee\":\"B$SCAT\"").count(), 2);
}

#[test]
fn every_string_intrinsic_is_a_descriptor_when_assigned_to_a_fixed_field() {
    // Q45LE71 previously treated LEFT$(...) as an array access only when its
    // destination was a fixed-string UDT field.  The fixed-string path had a
    // hand-written list of descriptor functions which omitted LEFT$ and the
    // binary packers, even though the intrinsic table already owns that type.
    let module = parse(
        "type Pair\ntag as string * 3\nend type\ndim pair as Pair\ndim text as string\npair.tag = left$(text, 3)\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "fixed_intrinsic", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("\"callee\":\"B$LEFT\""));
    assert!(hir.contains("\"callee\":\"B$ASSN\""));
}

#[test]
fn classic_string_intrinsics_keep_distinct_typed_runtime_interfaces() {
    // The focused QB45 string cases used to fall through to array lookup for
    // RIGHT$, UCASE$, HEX$, OCT$, CVI and CVL.  Assert the observable runtime
    // entries and widths instead of merely accepting their syntax.
    let module = parse(
        "dim text as string\ndim i as integer\ndim l as long\ntext = right$(\"abcd\", 2) + ucase$(\"q\") + hex$(4660) + oct$(511)\ni = instr(2, \"abcabc\", \"bc\") + cvi(mki$(4660))\nl = cvl(mkl$(305419896))\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "classic_strings", Dialect::QuickBasic45, "qb45").unwrap();
    for callee in [
        "B$RGHT", "B$UCAS", "B$FHEX", "B$FOCT", "B$INS3", "B$FMKI", "B$FCVI", "B$FMKL", "B$FCVL",
    ] {
        assert!(
            hir.contains(&format!("\"callee\":\"{callee}\"")),
            "{callee}"
        );
    }
}

#[test]
fn floating_binary_packers_round_to_their_declared_storage_width() {
    let module = parse(
        "dim text as string\ndim x as double\ntext = mks$(x) + mkd$(x)\n",
        Dialect::Pds71,
    )
    .unwrap();
    let hir = compile(&module, "float_packers", Dialect::Pds71, "pds71").unwrap();
    assert!(hir.contains("\"callee\":\"B$FMKS\""));
    assert!(hir.contains("\"callee\":\"B$FMKD\""));
    assert!(hir.matches("\"op\":\"store\"").count() >= 2);
}

#[test]
fn mbf_option_remaps_pack_and_unpack_as_one_audited_mode() {
    // PDMBF expected Microsoft Binary Format bytes, but dropping /MBF made
    // an otherwise successful frontend silently select the IEEE entries.
    let module = parse(
        "dim text as string\ndim x as single\ndim y as double\ntext = mks$(x) + mkd$(y)\nx = cvs(text)\ny = cvd(text)\n",
        Dialect::Pds71,
    )
    .unwrap();
    let ordinary = compile_with_options(
        &module,
        "ieee",
        Dialect::Pds71,
        "pds71",
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    let mbf = compile_with_options(
        &module,
        "mbf",
        Dialect::Pds71,
        "pds71",
        false,
        false,
        false,
        false,
        true,
        false,
    )
    .unwrap();
    for callee in ["B$FMKS", "B$FMKD", "B$FCVS", "B$FCVD"] {
        assert!(ordinary.contains(&format!("\"callee\":\"{callee}\"")));
        assert!(!mbf.contains(&format!("\"callee\":\"{callee}\"")));
    }
    for callee in ["B$FMSF", "B$FMDF", "B$MCVS", "B$MCVD"] {
        assert!(mbf.contains(&format!("\"callee\":\"{callee}\"")));
        assert!(!ordinary.contains(&format!("\"callee\":\"{callee}\"")));
    }
    // Both unpackers return an AX pointer into the runtime accumulator; a
    // separate load makes the memory result explicit in HIR.
    assert!(mbf.matches("\"op\":\"load\"").count() >= 2);
}

#[test]
fn dynamic_udt_field_address_stays_far_pointer_arithmetic() {
    let module = parse(
        "type Pixel\nred as string * 1\ngreen as string * 1\nend type\nredim pixels(0 to 2) as Pixel\npixels(0).green = chr$(0)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "dynamic_fixed_fields", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"op\":\"pointer_offset\""));
    assert!(hir.contains("\"callee\":\"B$ASSN\""));
}

#[test]
fn byval_float_is_rounded_to_declared_stack_width() {
    let module = parse(
        "declare sub consume (byval x as single, byval y as double)\ndim x as single\ndim y as double\nconsume x * 2, y + 1\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "byval_float", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"CONSUME\""));
    assert!(hir.matches("\"op\":\"store\"").count() >= 2);
    assert!(hir.contains("\"operands\":[{\"place\":"));
}

#[test]
fn erase_of_array_parameter_uses_its_incoming_descriptor() {
    let module = parse(
        "sub release(items() as long)\nerase items\nend sub\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "erase_parameter", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$ERAS\""));
}

#[test]
fn environ_selects_string_or_integer_runtime_entry() {
    let module = parse(
        "dim a as string\ndim b as string\na = environ$(\"BLASTER\")\nb = environ$(1)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "environ", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FEVS\""));
    assert!(hir.contains("\"callee\":\"B$FEVI\""));
}

#[test]
fn open_append_preserves_runtime_mode_bits() {
    let module = parse(
        "dim f as integer\nopen \"trace.txt\" for append as #f\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "open_append", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$OPEN\""));
    assert!(hir.contains("\"type\":1,\"value\":8"));
}

#[test]
fn bload_and_bsave_use_the_measured_runtime_interfaces() {
    let module = parse(
        "bsave \"DATA.BSV\", 12, 4\nbload \"DATA.BSV\"\nbload \"DATA.BSV\", 20\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "binary_memory", Dialect::QuickBasic45, "qb45").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$BSAV\"").count(), 1);
    assert_eq!(hir.matches("\"callee\":\"B$BLOD\"").count(), 2);
    assert!(hir.contains("\"type\":1,\"value\":0"));
    assert!(hir.contains("\"type\":1,\"value\":1"));
}

#[test]
fn print_keeps_destination_item_types_and_terminators() {
    let module = parse(
        "dim f as integer\ndim x as single\ndim y as single\nprint \"ready\"\nprint #f, x, y\nprint #f, \"partial\";\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "print", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$PESD\""));
    assert!(hir.contains("\"callee\":\"B$CHOU\""));
    assert!(hir.contains("\"callee\":\"B$PCR4\""));
    assert!(hir.contains("\"callee\":\"B$PER4\""));
    assert!(hir.contains("\"callee\":\"B$PSSD\""));
    assert!(hir.contains("\"callee\":\"B$PEOS\""));
}

#[test]
fn disk_input_keeps_far_destinations_and_string_width() {
    let module = parse(
        "dim f as integer\ndim x as single\ndim y as double\ndim text as string\ninput #f, x, y, text\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "input_file", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$DSKI\""));
    assert!(hir.contains("\"callee\":\"B$RDR4\""));
    assert!(hir.contains("\"callee\":\"B$RDR8\""));
    assert!(hir.contains("\"callee\":\"B$RDSD\""));
    assert!(hir.contains("\"callee\":\"B$PEOS\""));
}

#[test]
fn dir_keeps_the_vbdos_search_boundary_and_null_continuation() {
    let module = parse(
        "dim pattern as string\ndim first as string\ndim nextOne as string\nfirst = dir$(pattern)\nnextOne = dir$(\"\")\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "dir", Dialect::VbDos, "vbdos").unwrap();
    assert_eq!(hir.matches("\"callee\":\"B$FDR1\"").count(), 2);
    assert!(hir.contains("\"type\":1,\"value\":0"));
    assert!(hir.contains("\"callee\":\"B$SASS\""));
}

#[test]
fn terminal_statements_keep_the_measured_vbdos_call_shapes() {
    let module = parse("screen 0\nwidth 80, 25\nsleep\nend\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "terminal", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$CSCN\""));
    assert!(hir.contains("\"callee\":\"B$WIDT\""));
    assert!(hir.contains("\"callee\":\"B$SLEP\""));
    assert!(hir.contains("\"callee\":\"B$CEND\""));

    // QGL uses SYSTEM for its self-check exit. VBDOS lowers it through
    // the same B$CEND process-termination entry as END in a compiled EXE.
    let module = parse("system\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "system", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$CEND\""));

    // QGL's deterministic and TIMER seeds both pass an R8 value. BC's raw
    // MAIN.OBJ pushes 3ff00000:00000000 for RANDOMIZE 1 before B$RNZP.
    let module = parse("randomize 1\n", Dialect::VbDos).unwrap();
    let hir = compile(&module, "randomize", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$RNZP\""));
    assert!(hir.contains("\"type\":4"));
}

#[test]
fn redim_preserves_a_declared_fixed_string_element_type() {
    let module = parse(
        "dim shared labels() as string * 12\nredim labels(8) as string * 12\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "fixed_redim", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$RDIM\""));
    assert!(hir.contains("\"type\":1,\"value\":12"));
}

#[test]
fn fre_keeps_the_heap_query_as_an_effectful_long_call() {
    let module = parse(
        "dim available as long\navailable = fre(-1)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "fre", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$FRI2\""));
    assert!(hir.contains("\"type\":2"));
}

#[test]
fn procedure_calls_resolve_to_semantic_symbol_ids() {
    let module = parse(
        "declare function twice(byval x as integer) as integer\ndim y as integer\ny = twice(3)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "symbols", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callables\":[{\"arrays\":[false],\"by_value\":[true],\"defined\":false,\"id\":1,\"name\":\"TWICE\""));
    assert!(hir.contains("\"callee\":1,\"cleanup\":\"callee\""));
}

#[test]
fn on_error_resolves_to_function_side_metadata() {
    let module = parse(
        "on error goto handler\nend\nhandler:\nprint err, erl\nend\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "errors", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"error_handler\":2"));
    assert!(hir.contains("\"callee\":\"B$FERR\""));
    assert!(hir.contains("\"callee\":\"B$FERL\""));
}

#[test]
fn eqv_negates_one_xor_without_pre_negating_the_left_operand() {
    // Q45LG47 printed `FAIL logical eqv`: HIR formed NOT((NOT left) XOR
    // right), which is not BASIC's NOT(left XOR right).
    let module = parse(
        "dim leftValue as integer\n\
         dim rightValue as integer\n\
         dim answer as integer\n\
         answer = leftValue eqv rightValue\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "eqv", Dialect::QuickBasic45, "qb45").unwrap();
    assert_eq!(hir.matches("\"op\":\"xor\"").count(), 1);
    assert_eq!(hir.matches("\"op\":\"not\"").count(), 1);
}

#[test]
fn qb_intrinsics_resolve_through_one_declarative_catalogue() {
    use qbfront::intrinsics::{find, Effect, Lowering, ResultClass};

    let sine = find("SIN", Dialect::QuickBasic45).expect("SIN is a QB intrinsic");
    assert_eq!(sine.lowering, Lowering::Sin);
    assert_eq!(sine.effect, Effect::Pure);
    assert!(sine.accepts(1));
    assert!(!sine.accepts(2));

    let directory = find("DIR", Dialect::VbDos).expect("DIR$ is a VB-DOS intrinsic");
    assert_eq!(directory.result, ResultClass::String);
    assert_eq!(directory.effect, Effect::Runtime);
    assert_eq!(directory.lowering, Lowering::Directory);
    assert!(find("PLAYERTHINK", Dialect::VbDos).is_none());
}

#[test]
fn def_seg_and_peek_share_the_runtime_segment_cell_across_procedures() {
    let module = parse(
        "sub selectSegment(byval segment as integer)\n\
         def seg = segment\n\
         end sub\n\
         function readByte(byval offset as long) as integer\n\
         readByte = peek(offset)\n\
         end function\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "defseg_shared", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"name\":\"b$seg\""));
    assert!(hir.contains("\"storage\":\"external\""));
    assert!(!hir.contains("\"callee\":\"B$DSEG\""));
}

#[test]
fn space_string_keeps_the_measured_vbdos_runtime_boundary() {
    let module = parse(
        "dim width as integer\ndim row as string\nrow = space$(width)\n",
        Dialect::VbDos,
    )
    .unwrap();
    let hir = compile(&module, "space", Dialect::VbDos, "vbdos").unwrap();
    assert!(hir.contains("\"callee\":\"B$SPAC\""));
    assert!(hir.contains("\"callee\":\"B$SASS\""));
}

#[test]
fn def_type_governs_the_procedures_that_follow_it() {
    // qbdemo puts DEFDBL before SUB fracline and DEFINT after it; applying
    // every module DEFtype at once typed fracline's parameters INTEGER and
    // rejected its DECLARE with "declaration and definition do not agree".
    let module = parse(
        "declare sub fracline (y%, y1#)\r\n\
         defint a-z\r\nfracline 1, 2\r\n\
         defdbl a-z\r\nsub fracline (y%, y1)\r\nprint y1\r\nend sub\r\n\
         defint a-z\r\nsub other (i)\r\nprint i\r\nend sub\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    // BC 4.5's listing prints fracline's y1 with B$PER8.
    let hir = compile(&module, "deftype_position", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("B$PER8"), "{hir}");
}

#[test]
fn a_declared_function_suffix_matches_its_default_typed_definition() {
    // oimad declares `FUNCTION DMADone% (lengy%)` and defines it under
    // DEFINT as `FUNCTION DMADone (lengy)`; the callee kept the suffix and
    // the two were rejected as disagreeing.
    let module = parse(
        "declare function dmadone% (lengy%)\r\ndefint a-z\r\nprint dmadone(1)\r\n\
         function dmadone (lengy)\r\ndmadone = lengy\r\nend function\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    compile(&module, "function_suffix", Dialect::QuickBasic45, "qb45").unwrap();
}

#[test]
fn a_name_and_colon_after_the_start_of_a_line_is_a_call_not_a_label() {
    // deedlines repeats `IF xit% = 1 THEN xit% = 2: getpal: ...`; taking
    // `getpal:` for a label failed with "duplicate label GETPAL". BC 4.5's
    // listing calls G at both sites.
    let module = parse(
        "declare sub g ()\r\nx% = 1\r\n\
         if x% = 1 then x% = 2: g: print 1\r\n\
         x% = 3: g: print 2\r\n\
         g: print 3\r\n\
         sub g\r\nend sub\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "mid_line_call", Dialect::QuickBasic45, "qb45").unwrap();
    assert_eq!(
        module
            .statements
            .iter()
            .filter(|one| matches!(one, Statement::Label(..)))
            .count(),
        1,
        "only the line-initial g: is a label"
    );
    assert_eq!(hir.matches("\"callee\":\"G\"").count(), 2, "{hir}");
}

#[test]
fn a_scalar_and_an_array_may_share_a_name() {
    // deedlines DIMs g%() and uses scalar g% beside it; one namespace
    // rejected `g% = 0` with "array G% requires subscripts".
    let module = parse(
        "dim g%(5)\r\ng% = 7\r\ng%(1) = g% + 1\r\nprint g%, g%(1), ubound(g%)\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "shared_name", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(
        hir.contains("\"extent\":12,\"id\":1,\"name\":\"G%\""),
        "{hir}"
    );
    assert!(
        hir.contains("\"extent\":2,\"id\":3,\"name\":\"G%\""),
        "{hir}"
    );
}

#[test]
fn a_procedure_sees_only_shared_module_variables() {
    // deedlines' SUBs begin `SHARED kb%, px%, ...`, which did not parse.
    // Every procedure also saw every module variable: gorillas' DoSun
    // wrote the module's x, which QB keeps local without SHARED.
    let module = parse(
        "x% = 5\r\nnamed\r\nunnamed\r\n\
         sub named\r\nshared x%, onlyhere%\r\nx% = onlyhere%\r\nend sub\r\n\
         sub unnamed\r\nx% = 6\r\nend sub\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "shared_statement", Dialect::QuickBasic45, "qb45").unwrap();
    // NAMED stores to the module's X% (place 1); UNNAMED to its own.
    assert_eq!(
        hir.matches("\"op\":\"store\",\"operands\":[{\"place\":1,")
            .count(),
        2,
        "{hir}"
    );
    assert_eq!(
        hir.matches("\"name\":\"X%\",\"offset\":-2,\"storage\":\"local\"")
            .count(),
        1,
        "{hir}"
    );
}

#[test]
fn graphics_put_and_get_take_an_array_element() {
    // oimad draws with `PUT (x, y), mask(1500), AND`; BC 4.5 passes the
    // element's far address where the bare form passes the data start.
    let module = parse(
        "defint a-z\r\ndim m(2000)\r\nscreen 13\r\n\
         put (1, 2), m(1500), and\r\nget (0, 0)-(9, 9), m(i)\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "graphics_element", Dialect::QuickBasic45, "qb45").unwrap();
    assert!(hir.contains("B$GPUT") && hir.contains("B$GGET"), "{hir}");
}

#[test]
fn a_power_of_a_variable_base_covers_the_whole_domain() {
    // deedlines' `SQR(...) ^ 1.5` and `^ meg` were refused: only a positive
    // constant base had a lowering. B$POW4 raises error 5 for 0 ^ -1 and a
    // negative base under a fractional exponent.
    let module = parse(
        "b! = -2: e! = 3\r\nr! = b! ^ e!\r\ns# = sqr(2#) ^ 1.5\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "general_power", Dialect::QuickBasic45, "qb45").unwrap();
    assert_eq!(hir.matches("\"op\":\"fexp2\"").count(), 4, "{hir}");
    assert_eq!(hir.matches("\"callee\":\"B$SERR\"").count(), 2, "{hir}");
}

#[test]
fn port_io_statements_reach_the_ports_and_bound_their_byte() {
    // OUT/INP/WAIT had no lowering. BC folds a constant byte and refuses
    // one out of range with "Math overflow"; the frontend emitted 298 as-is.
    let module = parse(
        "p = &H3C8: OUT p, 40: a = INP(p + 1): WAIT &H3DA, 8, 8\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "ports", Dialect::QuickBasic45, "qb45").unwrap();
    assert_eq!(hir.matches("\"op\":\"port_out\"").count(), 1, "{hir}");
    assert_eq!(hir.matches("\"op\":\"port_in\"").count(), 2, "{hir}");
    let module = parse("OUT 5, 298\r\n", Dialect::QuickBasic45).unwrap();
    let error =
        compile(&module, "overflow", Dialect::QuickBasic45, "qb45").expect_err("298 is no byte");
    assert!(format!("{error:?}").contains("Math overflow"), "{error:?}");
}

#[test]
fn a_dynamic_dim_allocates_where_it_runs() {
    // qbdemo's `$DYNAMIC` DIMs ran at entry, ahead of SCREEN 13, which
    // then had no memory: "Illegal function call". Bounds read at entry
    // also saw n before its assignment.
    let module = parse(
        "'$DYNAMIC\r\nscreen 13\r\nn% = 5\r\ndim a%(n%)\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "dynamic_dim", Dialect::QuickBasic45, "qb45").unwrap();
    let screen = hir.find("\"callee\":\"B$CSCN\"").expect("SCREEN");
    let store = hir.find("\"value\":5}").expect("n% = 5");
    let dim = hir.find("\"callee\":\"B$DDIM\"").expect("DIM");
    assert!(screen < dim && store < dim, "{hir}");
}

#[test]
fn a_parenthesized_argument_passes_a_copy() {
    // qbdemo's `drawbob (bob(bobptr))` advanced the array element itself,
    // so `undrawbob` erased the wrong trail: parentheses make a value.
    let module = parse(
        "declare sub inc (v%)\r\na% = 1: b% = 1\r\ninc (a%)\r\ninc b%\r\nprint a%; b%\r\n\
         sub inc (v%)\r\nv% = v% + 1\r\nend sub\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "grouped", Dialect::QuickBasic45, "qb45").unwrap();
    let address = |place: u32| {
        hir.matches(&format!(
            "\"op\":\"address\",\"operands\":[{{\"place\":{place},\"tag\":\"place\"}}]"
        ))
        .count()
    };
    assert_eq!((address(1), address(2)), (0, 1), "{hir}");
}

#[test]
fn graphics_put_passes_the_runtime_action_codes() {
    // oimad's `PUT ..., text(1500), OR` drew XOR: OR was passed as 4 and XOR
    // as 5, from getput.asm's header comment. Its PutGetInit table and the
    // QB45 /A listing agree: OR 0, AND 1, PRESET 2, PSET 3, XOR 4 (default).
    let module = parse(
        "defint a-z\r\ndim a(100)\r\nscreen 13\r\nput (1, 1), a, pset\r\nput (1, 1), a, preset\r\n\
         put (1, 1), a, and\r\nput (1, 1), a, or\r\nput (1, 1), a, xor\r\nput (1, 1), a\r\n",
        Dialect::QuickBasic45,
    )
    .unwrap();
    let hir = compile(&module, "put_actions", Dialect::QuickBasic45, "qb45").unwrap();
    let actions: Vec<&str> = hir
        .split("\"callee\":\"B$GPUT\"")
        .skip(1)
        .map(|call| {
            let operands = &call[..call.find("}]").expect("operands end")];
            &operands[operands.rfind("\"value\":").expect("action") + 8..]
        })
        .collect();
    assert_eq!(actions, ["3", "2", "1", "0", "4", "4"], "{hir}");
}
