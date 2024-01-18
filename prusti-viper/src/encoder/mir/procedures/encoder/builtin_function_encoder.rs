use super::*;

pub(super) trait BuiltinFuncAppEncoder<'p, 'v, 'tcx> {
    #[allow(clippy::too_many_arguments)]
    fn try_encode_builtin_call(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        span: Span,
        called_def_id: DefId,
        call_substs: SubstsRef<'tcx>,
        args: &[mir::Operand<'tcx>],
        destination: mir::Place<'tcx>,
        target: &Option<mir::BasicBlock>,
        cleanup: &Option<mir::BasicBlock>,
        original_lifetimes: &mut BTreeSet<String>,
        derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<bool>;
}

impl<'p, 'v, 'tcx> BuiltinFuncAppEncoder<'p, 'v, 'tcx> for super::ProcedureEncoder<'p, 'v, 'tcx> {
    #[tracing::instrument(level = "debug", skip_all, fields(called_def_id = ?called_def_id))]
    fn try_encode_builtin_call(
        &mut self,
        block_builder: &mut BasicBlockBuilder,
        location: mir::Location,
        span: Span,
        called_def_id: DefId,
        call_substs: SubstsRef<'tcx>,
        args: &[mir::Operand<'tcx>],
        destination: mir::Place<'tcx>,
        target: &Option<mir::BasicBlock>,
        cleanup: &Option<mir::BasicBlock>,
        original_lifetimes: &mut BTreeSet<String>,
        derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
    ) -> SpannedEncodingResult<bool> {
        let full_called_function_name = self
            .encoder
            .env()
            .name
            .get_absolute_item_name(called_def_id);

        let make_manual_assign = |encoder: &mut Self,
                                  block_builder: &mut BasicBlockBuilder,
                                  original_lifetimes: &mut BTreeSet<String>,
                                  derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
                                  rhs_gen: &mut dyn FnMut(
            _,
            Vec<vir_high::Expression>,
            _,
        ) -> vir_high::Expression|
         -> SpannedEncodingResult<()> {
            let (target_place, target_block) = (destination, target.unwrap());
            let position = encoder
                .encoder
                .error_manager()
                .register_error(span, ErrorCtxt::WritePlace, encoder.def_id)
                .into();
            let encoded_target_place = encoder
                .encoder
                .encode_place_high(encoder.mir, target_place, None)?
                .set_default_position(position);
            let encoded_args = args
                .iter()
                .map(|arg| encoder.encode_statement_operand_no_refs(block_builder, location, arg))
                .collect::<Result<Vec<_>, _>>()?;
            for encoded_arg in encoded_args.iter() {
                let statement = vir_high::Statement::consume_no_pos(encoded_arg.clone());
                block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                    statement,
                    span,
                    ErrorCtxt::ProcedureCall,
                    encoder.def_id,
                )?);
            }
            let target_place_local = if let Some(target_place_local) = target_place.as_local() {
                target_place_local
            } else {
                unimplemented!()
            };
            let size = encoder.encoder.encode_type_size_expression(
                encoder
                    .encoder
                    .get_local_type(encoder.mir, target_place_local)?,
            )?;
            let target_memory_block =
                vir_high::Predicate::memory_block_stack_no_pos(encoded_target_place.clone(), size);
            block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                vir_high::Statement::exhale_predicate_no_pos(target_memory_block),
                span,
                ErrorCtxt::ProcedureCall,
                encoder.def_id,
            )?);
            let inhale_statement = vir_high::Statement::inhale_predicate_no_pos(
                vir_high::Predicate::owned_non_aliased_no_pos(encoded_target_place.clone()),
            );
            block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                inhale_statement,
                span,
                ErrorCtxt::ProcedureCall,
                encoder.def_id,
            )?);
            let type_arguments = encoder
                .encoder
                .encode_generic_arguments_high(called_def_id, call_substs)
                .with_span(span)?;

            let encoded_arg_expressions =
                encoded_args.into_iter().map(|arg| arg.expression).collect();

            let target_type = encoded_target_place.get_type().clone();

            let expression = vir_high::Expression::equals(
                encoded_target_place,
                rhs_gen(type_arguments, encoded_arg_expressions, target_type),
            );
            let assume_statement = encoder.encoder.set_statement_error_ctxt(
                vir_high::Statement::assume_no_pos(expression),
                span,
                ErrorCtxt::UnexpectedAssumeMethodPostcondition,
                encoder.def_id,
            )?;
            block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                assume_statement,
                span,
                ErrorCtxt::ProcedureCall,
                encoder.def_id,
            )?);
            encoder.encode_lft_for_block(
                target_block,
                location,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
            )?;
            encoder.add_predecessor(location.block, target_block)?;
            let target_label = encoder.encode_basic_block_label(target_block);
            let successor = vir_high::Successor::Goto(target_label);
            block_builder.set_successor_jump(successor);
            Ok(())
        };

        let make_builtin_call = |encoder: &mut Self,
                                 block_builder: &mut BasicBlockBuilder,
                                 original_lifetimes: &mut BTreeSet<String>,
                                 derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
                                 function|
         -> SpannedEncodingResult<()> {
            make_manual_assign(
                encoder,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                &mut |ty_args, args, target_ty| {
                    vir_high::Expression::builtin_func_app_no_pos(
                        function, ty_args, args, target_ty,
                    )
                },
            )?;
            Ok(())
        };

        let make_binop = |encoder: &mut Self,
                          block_builder: &mut BasicBlockBuilder,
                          original_lifetimes: &mut BTreeSet<String>,
                          derived_lifetimes: &mut BTreeMap<String, BTreeSet<String>>,
                          op_kind|
         -> SpannedEncodingResult<()> {
            make_manual_assign(
                encoder,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                &mut |_ty_args, args, _target_ty| {
                    vir_high::Expression::binary_op_no_pos(
                        op_kind,
                        args[0].clone(),
                        args[1].clone(),
                    )
                },
            )?;
            Ok(())
        };

        if let Some(op_name) = full_called_function_name
            .as_str()
            .strip_prefix("std::ops::")
            .or_else(|| {
                full_called_function_name
                    .as_str()
                    .strip_prefix("core::ops::")
            })
        {
            let lhs = self
                .encode_statement_operand_no_refs(block_builder, location, &args[0])?
                .expression;
            if lhs.get_type() == &vir_high::Type::Int(vir_high::ty::Int::Unbounded) {
                use vir_high::BinaryOpKind::*;
                let ops = [
                    ("Add::add", Add),
                    ("Sub::sub", Sub),
                    ("Mul::mul", Mul),
                    ("Div::div", Div),
                    ("Rem::rem", Mod),
                ];
                for op in ops {
                    if op_name == op.0 {
                        make_binop(
                            self,
                            block_builder,
                            original_lifetimes,
                            derived_lifetimes,
                            op.1,
                        )?;
                        return Ok(true);
                    }
                }
            }
        }

        match full_called_function_name.as_str() {
            "std::rt::begin_panic" | "core::panicking::panic" | "core::panicking::panic_fmt" => {
                let panic_message = format!("{:?}", args[0]);
                let panic_cause = self.encoder.encode_panic_cause(span)?;
                if self.check_panics {
                    block_builder.add_comment(format!("Rust panic - {panic_message}"));
                    block_builder.add_statement(self.encoder.set_statement_error_ctxt(
                        vir_high::Statement::assert_no_pos(false.into()),
                        span,
                        ErrorCtxt::Panic(panic_cause),
                        self.def_id,
                    )?);
                } else {
                    debug!("Absence of panic will not be checked")
                }
                assert!(target.is_none());
                if let Some(cleanup) = cleanup {
                    let successor =
                        vir_high::Successor::Goto(self.encode_basic_block_label(*cleanup));
                    block_builder.set_successor_jump(successor);
                } else {
                    unimplemented!();
                }
            }
            "prusti_contracts::prusti_take_lifetime" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::TakeLifetime,
            )?,
            "prusti_contracts::prusti_set_lifetime_for_raw_pointer_reference_casts" => {
                // Do nothing, this function is used only by the drop
                // elaboration pass.
            }
            "prusti_contracts::Int::new"
            | "prusti_contracts::Int::new_usize"
            | "prusti_contracts::Int::new_isize" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::NewInt,
            )?,
            "prusti_contracts::Int::to_usize" | "prusti_contracts::Int::to_isize" => {
                let (source_type, destination_type) = match full_called_function_name.as_str() {
                    "prusti_contracts::Int::new" => (
                        vir_high::Type::Int(vir_high::ty::Int::I64),
                        vir_high::Type::Int(vir_high::ty::Int::Unbounded),
                    ),
                    "prusti_contracts::Int::new_usize" => (
                        vir_high::Type::Int(vir_high::ty::Int::Usize),
                        vir_high::Type::Int(vir_high::ty::Int::Unbounded),
                    ),
                    "prusti_contracts::Int::new_isize" => (
                        vir_high::Type::Int(vir_high::ty::Int::Isize),
                        vir_high::Type::Int(vir_high::ty::Int::Unbounded),
                    ),
                    "prusti_contracts::Int::to_usize" => (
                        vir_high::Type::Int(vir_high::ty::Int::Unbounded),
                        vir_high::Type::Int(vir_high::ty::Int::Usize),
                    ),
                    "prusti_contracts::Int::to_isize" => (
                        vir_high::Type::Int(vir_high::ty::Int::Unbounded),
                        vir_high::Type::Int(vir_high::ty::Int::Isize),
                    ),
                    _ => unreachable!("no further int functions"),
                };
                let ty_args = vec![source_type, destination_type];
                make_manual_assign(
                    self,
                    block_builder,
                    original_lifetimes,
                    derived_lifetimes,
                    &mut |_, args, target_ty| {
                        vir_high::Expression::builtin_func_app_no_pos(
                            vir_high::BuiltinFunc::CastIntToInt,
                            ty_args.clone(),
                            args,
                            target_ty,
                        )
                    },
                )?
            }
            "prusti_contracts::Map::<K, V>::empty" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::EmptyMap,
            )?,
            "prusti_contracts::Map::<K, V>::insert" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::UpdateMap,
            )?,
            "prusti_contracts::Map::<K, V>::delete" => {
                unimplemented!()
            }
            "prusti_contracts::Map::<K, V>::len" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::MapLen,
            )?,
            "prusti_contracts::Map::<K, V>::contains" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::MapContains,
            )?,
            "prusti_contracts::Map::<K, V>::lookup" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::LookupMap,
            )?,
            "prusti_contracts::Seq::<T>::empty" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::EmptySeq,
            )?,
            "prusti_contracts::Seq::<T>::single" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::SingleSeq,
            )?,
            "prusti_contracts::Seq::<T>::concat" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::ConcatSeq,
            )?,
            "prusti_contracts::Seq::<T>::lookup" => make_builtin_call(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                vir_high::BuiltinFunc::LookupSeq,
            )?,
            "prusti_contracts::Ghost::<T>::new" => make_manual_assign(
                self,
                block_builder,
                original_lifetimes,
                derived_lifetimes,
                &mut |_, args, _| args[0].clone(),
            )?,
            "prusti_contracts::snapshot_equality" => {
                unreachable!();
            }
            "std::ops::Index::index" | "core::ops::Index::index" => {
                let lhs = self
                    .encode_statement_operand_no_refs(block_builder, location, &args[0])?
                    .expression;
                let typ = match lhs.get_type() {
                    vir_high::Type::Reference(vir_high::ty::Reference { target_type, .. }) => {
                        &**target_type
                    }
                    _ => unreachable!(),
                };
                match typ {
                    vir_high::Type::Sequence(..) => make_builtin_call(
                        self,
                        block_builder,
                        original_lifetimes,
                        derived_lifetimes,
                        vir_high::BuiltinFunc::LookupSeq,
                    )?,
                    vir_high::Type::Map(..) => make_builtin_call(
                        self,
                        block_builder,
                        original_lifetimes,
                        derived_lifetimes,
                        vir_high::BuiltinFunc::LookupMap,
                    )?,
                    _ => return Ok(false),
                }
            }
            "std::cmp::PartialEq::eq" => {
                let lhs = self
                    .encode_statement_operand_no_refs(block_builder, location, &args[0])?
                    .expression;
                if matches!(
                    lhs.get_type(),
                    vir_high::Type::Reference(vir_high::ty::Reference {
                        target_type:
                            box vir_high::Type::Int(vir_high::ty::Int::Unbounded)
                            | box vir_high::Type::Sequence(..)
                            | box vir_high::Type::Map(..),
                        ..
                    })
                ) {
                    make_binop(
                        self,
                        block_builder,
                        original_lifetimes,
                        derived_lifetimes,
                        vir_high::BinaryOpKind::EqCmp,
                    )?;
                    return Ok(true);
                } else {
                    return Ok(false);
                }
            }
            "std::mem::forget" | "core::mem::forget" => {
                assert_eq!(args.len(), 1);
                let operand = &args[0];
                let mir_place = match operand {
                    mir::Operand::Move(place) => {
                        *place
                    }
                    mir::Operand::Copy(_) => {
                        unimplemented!("operand {operand:?} is copy");
                    }
                    mir::Operand::Constant(_) => unimplemented!("operand {operand:?} is constant"),
                };
                let place = self.encoder
                .encode_place_high(self.mir, mir_place, Some(span))?;
                let mut deallocation = Vec::new(); // TODO: Clean-up.

                {
                    eprintln!("HACK: Removing move assignment before mem::forget");
                    // FIXME: This deletes the move assignment that causes the fold before mem::forget.
                    let statements = block_builder.borrow_statements_hack();
                    let mut i = statements.len();
                    while i > 0 {
                        i -= 1;
                        let statement = &statements[i];
                        eprintln!("statement: {statement:?}");
                        if let vir_high::Statement::Assign(assign) = statement {
                            eprintln!("assign: {assign:?}");
                            unimplemented!("assign: {assign}");
                            statements.remove(i);
                            break;
                        }
                    }
                    eprintln!("HACK-END: Removing move assignment before mem::forget");
                }
                
                let position = self
                    .encoder
                    .error_manager()
                    .register_error(span, ErrorCtxt::MemForget, self.def_id)
                    .into();
                self.add_drop_impl_deallocation_statements(&mut deallocation, position, place)?;
                let local = mir_place.as_local().unwrap();
                let memory_block = self
                    .encoder
                    .encode_memory_block_for_local(self.mir, local)?;
                let alloc_statement = vir_high::Statement::inhale_predicate_no_pos(
                    memory_block,
                );
                deallocation.push(self.encoder.set_surrounding_error_context_for_statement(
                    alloc_statement,
                    position,
                    ErrorCtxt::MemForget,
                )?);

                let encoder = self;

                // FIXME: This code is copy-paste.
                let (target_place, target_block) = (destination, target.unwrap());
                let position = encoder
                    .encoder
                    .error_manager()
                    .register_error(span, ErrorCtxt::WritePlace, encoder.def_id)
                    .into();
                let encoded_target_place = encoder
                    .encoder
                    .encode_place_high(encoder.mir, target_place, None)?
                    .set_default_position(position);
                block_builder.add_statements(deallocation);
                // let encoded_args = args
                //     .iter()
                //     .map(|arg| encoder.encode_statement_operand_no_refs(block_builder, location, arg))
                //     .collect::<Result<Vec<_>, _>>()?;
                // for encoded_arg in encoded_args.iter() {
                //     let statement = vir_high::Statement::consume_no_pos(encoded_arg.clone());
                //     block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                //         statement,
                //         span,
                //         ErrorCtxt::ProcedureCall,
                //         encoder.def_id,
                //     )?);
                // }
                let target_place_local = if let Some(target_place_local) = target_place.as_local() {
                    target_place_local
                } else {
                    unimplemented!()
                };
                let size = encoder.encoder.encode_type_size_expression(
                    encoder
                        .encoder
                        .get_local_type(encoder.mir, target_place_local)?,
                )?;
                let target_memory_block =
                    vir_high::Predicate::memory_block_stack_no_pos(encoded_target_place.clone(), size);
                block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                    vir_high::Statement::exhale_predicate_no_pos(target_memory_block),
                    span,
                    ErrorCtxt::ProcedureCall,
                    encoder.def_id,
                )?);
                let inhale_statement = vir_high::Statement::inhale_predicate_no_pos(
                    vir_high::Predicate::owned_non_aliased_no_pos(encoded_target_place.clone()),
                );
                block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                    inhale_statement,
                    span,
                    ErrorCtxt::ProcedureCall,
                    encoder.def_id,
                )?);
                // let type_arguments = encoder
                //     .encoder
                //     .encode_generic_arguments_high(called_def_id, call_substs)
                //     .with_span(span)?;
    
                // let encoded_arg_expressions =
                //     encoded_args.into_iter().map(|arg| arg.expression).collect();
    
                // let target_type = encoded_target_place.get_type().clone();
    
                // let expression = vir_high::Expression::equals(
                //     encoded_target_place,
                //     rhs_gen(type_arguments, encoded_arg_expressions, target_type),
                // );
                // let assume_statement = encoder.encoder.set_statement_error_ctxt(
                //     vir_high::Statement::assume_no_pos(expression),
                //     span,
                //     ErrorCtxt::UnexpectedAssumeMethodPostcondition,
                //     encoder.def_id,
                // )?;
                // block_builder.add_statement(encoder.encoder.set_statement_error_ctxt(
                //     assume_statement,
                //     span,
                //     ErrorCtxt::ProcedureCall,
                //     encoder.def_id,
                // )?);
                encoder.encode_lft_for_block(
                    target_block,
                    location,
                    block_builder,
                    original_lifetimes,
                    derived_lifetimes,
                )?;
                encoder.add_predecessor(location.block, target_block)?;
                let target_label = encoder.encode_basic_block_label(target_block);
                let successor = vir_high::Successor::Goto(target_label);
                block_builder.set_successor_jump(successor);

                return Ok(true);
            }
            _ => return Ok(false),
        };
        Ok(true)
    }
}
