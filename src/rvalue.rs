use crate::cil::{CILOp, CallSite, FieldDescriptor};
use crate::function_sig::FnSig;
use crate::operand::handle_operand;
use crate::place::deref_op;
use crate::r#type::{TyCache, Type};
use rustc_middle::{
    mir::{CastKind, NullOp, Place, Rvalue},
    ty::{adjustment::PointerCoercion, Instance, ParamEnv, Ty, TyCtxt, TyKind},
};
pub fn handle_rvalue<'tcx>(
    rvalue: &Rvalue<'tcx>,
    tyctx: TyCtxt<'tcx>,
    target_location: &Place<'tcx>,
    method: &rustc_middle::mir::Body<'tcx>,
    method_instance: Instance<'tcx>,
    tycache: &mut TyCache,
) -> Vec<CILOp> {
    let res = match rvalue {
        Rvalue::Use(operand) => {
            let res = handle_operand(operand, tyctx, method, method_instance, tycache);
            res
        }
        Rvalue::CopyForDeref(place) => {
            crate::place::place_get(place, tyctx, method, method_instance, tycache)
        }
        Rvalue::Ref(_region, _kind, place) => {
            crate::place::place_adress(place, tyctx, method, method_instance, tycache)
        }
        Rvalue::AddressOf(_mutability, place) => {
            crate::place::place_adress(place, tyctx, method, method_instance, tycache)
        }
        Rvalue::Cast(
            CastKind::PointerCoercion(PointerCoercion::MutToConstPointer) | CastKind::PtrToPtr,
            operand,
            dst,
        ) => {
            let target = crate::utilis::monomorphize(&method_instance, *dst, tyctx);
            let target_pointed_to = match target.kind() {
                TyKind::RawPtr(type_and_mut) => type_and_mut.ty,
                TyKind::Ref(_, inner, _) => *inner,
                _ => panic!("Type is not ptr {target:?}."),
            };
            let source =
                crate::utilis::monomorphize(&method_instance, operand.ty(method, tyctx), tyctx);
            let source_pointed_to = match source.kind() {
                TyKind::RawPtr(type_and_mut) => type_and_mut.ty,
                TyKind::Ref(_, inner, _) => *inner,
                _ => panic!("Type is not ptr {target:?}."),
            };
            let source_type = tycache.type_from_cache(source, tyctx, Some(method_instance));
            //let target_type = tycache.type_from_cache(target, tyctx, Some(method_instance));

            let ops = match (source_pointed_to.kind(), target_pointed_to.kind()) {
                (TyKind::Slice(_) | TyKind::Str, TyKind::Slice(_) | TyKind::Str) => {
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    res.push(CILOp::NewTMPLocal(source_type.into()));
                    res.push(CILOp::SetTMPLocal);
                    res.push(CILOp::LoadAddresOfTMPLocal);
                    res.push(CILOp::FreeTMPLocal);
                    res.extend(crate::place::deref_op(
                        crate::place::PlaceTy::Ty(target),
                        tyctx,
                        &method_instance,
                        tycache,
                    ));
                    res
                }
                (TyKind::Slice(_) | TyKind::Str, _) => {
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    //println!("Slice!");
                    res.push(CILOp::LDField(
                        FieldDescriptor::new(
                            source_type.as_dotnet().unwrap(),
                            Type::Ptr(Type::Void.into()),
                            "data_address".into(),
                        )
                        .into(),
                    ));
                    res
                }
                _ => handle_operand(operand, tyctx, method, method_instance, tycache),
            };
            //println!("casting {source:?} source_pointed_to:{source_pointed_to:?} to {target:?} target_pointed_to:{target_pointed_to:?}. ops:{ops:?}");
            ops
        }
        Rvalue::Cast(CastKind::PointerCoercion(PointerCoercion::Unsize), operand, target) => {
            let target = crate::utilis::monomorphize(&method_instance, *target, tyctx);
            let source =
                crate::utilis::monomorphize(&method_instance, operand.ty(method, tyctx), tyctx);
            let source_type = tycache.type_from_cache(source, tyctx, Some(method_instance));
            let target_type = tycache.type_from_cache(target, tyctx, Some(method_instance));
            let target_dotnet = target_type.as_dotnet().unwrap();
            let derefed_source = match source.kind() {
                TyKind::RawPtr(tpe) => tpe.ty,
                TyKind::Ref(_, inner, _) => *inner,
                _ => panic!("Non ptr type:{source:?}"),
            };
            let length = if let TyKind::Array(_, length) = derefed_source.kind() {
                crate::utilis::try_resolve_const_size(*length).unwrap()
            } else {
                panic!("Non array type:{source:?}")
            };
            //let element_type = tycache.type_from_cache(*element, tyctx, Some(method_instance));
            let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
            println!("Unsize casting {target:?} to {target:?}");
            res.extend([
                CILOp::NewTMPLocal(source_type.clone().into()),
                CILOp::SetTMPLocal,
                // a:*[T;n] = stack_top;
                CILOp::NewTMPLocal(target_type.into()),
                // b:Slice = unint();
                CILOp::LoadAddresOfTMPLocal,
                CILOp::LoadUnderTMPLocal(1),
                CILOp::STField(
                    FieldDescriptor::new(
                        target_dotnet.clone(),
                        Type::Ptr(Type::Void.into()),
                        "data_address".into(),
                    )
                    .into(),
                ),
                // b.data_address = (a as *mut RustVoid);
                CILOp::LoadAddresOfTMPLocal,
                CILOp::LdcI64(length as u64 as i64),
                CILOp::ConvUSize(false),
                CILOp::STField(
                    FieldDescriptor::new(target_dotnet, Type::USize, "metadata".into()).into(),
                ),
                // b.metadata = (length_i64 as usize);
                CILOp::LoadTMPLocal,
                // stack_top = b;
                CILOp::FreeTMPLocal,
                CILOp::FreeTMPLocal,
            ]);
            res
            //todo!("Array to slice {res:?}!")
            //
        }
        Rvalue::BinaryOp(binop, operands) => crate::binop::binop_unchecked(
            *binop,
            &operands.0,
            &operands.1,
            tyctx,
            method,
            method_instance,
            tycache,
        ),
        Rvalue::CheckedBinaryOp(binop, operands) => crate::checked_binop::binop_checked(
            *binop,
            &operands.0,
            &operands.1,
            tyctx,
            method,
            method_instance,
            tycache,
        ),
        Rvalue::UnaryOp(binop, operand) => {
            crate::unop::unop(*binop, operand, tyctx, method, method_instance, tycache)
        }
        Rvalue::Cast(CastKind::IntToInt, operand, target) => {
            let target = crate::utilis::monomorphize(&method_instance, *target, tyctx);
            let target = tycache.type_from_cache(target, tyctx, Some(method_instance));
            let src = operand.ty(&method.local_decls, tyctx);
            let src = crate::utilis::monomorphize(&method_instance, src, tyctx);
            let src = tycache.type_from_cache(src, tyctx, Some(method_instance));
            [
                handle_operand(operand, tyctx, method, method_instance, tycache),
                crate::casts::int_to_int(src, target),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        Rvalue::Cast(CastKind::FloatToInt, operand, target) => {
            let target = crate::utilis::monomorphize(&method_instance, *target, tyctx);
            let target = tycache.type_from_cache(target, tyctx, Some(method_instance));
            let src = operand.ty(&method.local_decls, tyctx);
            let src = crate::utilis::monomorphize(&method_instance, src, tyctx);
            let src = tycache.type_from_cache(src, tyctx, Some(method_instance));
            [
                handle_operand(operand, tyctx, method, method_instance, tycache),
                crate::casts::float_to_int(src, target),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        Rvalue::Cast(CastKind::IntToFloat, operand, target) => {
            let target = crate::utilis::monomorphize(&method_instance, *target, tyctx);
            let target = tycache.type_from_cache(target, tyctx, Some(method_instance));
            let src = operand.ty(&method.local_decls, tyctx);
            let src = crate::utilis::monomorphize(&method_instance, src, tyctx);
            let src = tycache.type_from_cache(src, tyctx, Some(method_instance));
            [
                handle_operand(operand, tyctx, method, method_instance, tycache),
                crate::casts::int_to_float(src, target),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        Rvalue::NullaryOp(op, ty) => match op {
            NullOp::SizeOf => {
                let ty = crate::utilis::monomorphize(&method_instance, *ty, tyctx);
                let ty = Box::new(tycache.type_from_cache(ty, tyctx, Some(method_instance)));
                vec![CILOp::SizeOf(ty)]
            }
            NullOp::AlignOf => {
                let ty = crate::utilis::monomorphize(&method_instance, *ty, tyctx);
                vec![
                    CILOp::LdcI64(align_of(ty, tyctx) as i64),
                    CILOp::ConvUSize(false),
                ]
            }
            NullOp::OffsetOf(_) => todo!("Unsuported nullary {op:?}!"),
        },
        Rvalue::Aggregate(aggregate_kind, field_index) => crate::aggregate::handle_aggregate(
            tyctx,
            target_location,
            method,
            aggregate_kind.as_ref(),
            field_index,
            method_instance,
            tycache,
        ),
        Rvalue::Cast(CastKind::Transmute, operand, dst) => {
            let dst = crate::utilis::monomorphize(&method_instance, *dst, tyctx);
            let dst_ty = dst;
            let dst = tycache.type_from_cache(dst, tyctx, Some(method_instance));
            let src = operand.ty(&method.local_decls, tyctx);
            let src = crate::utilis::monomorphize(&method_instance, src, tyctx);
            let src = tycache.type_from_cache(src, tyctx, Some(method_instance));
            match (&src, &dst) {
                (
                    Type::ISize | Type::USize | Type::Ptr(_),
                    Type::ISize | Type::USize | Type::Ptr(_),
                ) => handle_operand(operand, tyctx, method, method_instance, tycache),
                (Type::U16, Type::DotnetChar) => {
                    handle_operand(operand, tyctx, method, method_instance, tycache)
                }
                (Type::F64, Type::U64) => {
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    res.extend([
                        CILOp::NewTMPLocal(Type::Ptr(src.into()).into()),
                        CILOp::SetTMPLocal,
                        CILOp::LoadAddresOfTMPLocal,
                        CILOp::LDIndI64,
                        CILOp::FreeTMPLocal,
                    ]);
                    res
                }
                (Type::F32, Type::U32) => {
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    res.extend([
                        CILOp::NewTMPLocal(Type::Ptr(src.into()).into()),
                        CILOp::SetTMPLocal,
                        CILOp::LoadAddresOfTMPLocal,
                        CILOp::LDIndI32,
                        CILOp::FreeTMPLocal,
                    ]);
                    res
                }
                (Type::U32, Type::F32) => {
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    res.extend([
                        CILOp::NewTMPLocal(Type::Ptr(src.into()).into()),
                        CILOp::SetTMPLocal,
                        CILOp::LoadAddresOfTMPLocal,
                        CILOp::LDIndF32,
                        CILOp::FreeTMPLocal,
                    ]);
                    res
                }
                (Type::U64, Type::F64) => {
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    res.extend([
                        CILOp::NewTMPLocal(Type::Ptr(src.into()).into()),
                        CILOp::SetTMPLocal,
                        CILOp::LoadAddresOfTMPLocal,
                        CILOp::LDIndF64,
                        CILOp::FreeTMPLocal,
                    ]);
                    res
                }
                (_, _) => {
                    eprintln!(
                        "transmute from {src:?} to {dst:?} does not have special handling yet!"
                    );
                    let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
                    res.push(CILOp::NewTMPLocal(src.into()));
                    res.push(CILOp::SetTMPLocal);
                    res.push(CILOp::LoadAddresOfTMPLocal);
                    res.push(CILOp::FreeTMPLocal);
                    res.extend(crate::place::deref_op(
                        crate::place::PlaceTy::Ty(dst_ty),
                        tyctx,
                        &method_instance,
                        tycache,
                    ));
                    res
                }
            }
        }
        Rvalue::ShallowInitBox(operand, dst) => {
            let dst = crate::utilis::monomorphize(&method_instance, *dst, tyctx);
            let boxed_dst = Ty::new_box(tyctx, dst);
            //let dst = tycache.type_from_cache(dst, tyctx, Some(method_instance));
            let src = operand.ty(&method.local_decls, tyctx);
            let src = crate::utilis::monomorphize(&method_instance, src, tyctx);
            let src = tycache.type_from_cache(src, tyctx, Some(method_instance));
            let mut res = handle_operand(operand, tyctx, method, method_instance, tycache);
            res.push(CILOp::NewTMPLocal(
                crate::r#type::Type::Ptr(src.into()).into(),
            ));
            res.push(CILOp::SetTMPLocal);
            res.push(CILOp::LoadAddresOfTMPLocal);
            res.push(CILOp::FreeTMPLocal);
            res.extend(deref_op(boxed_dst.into(), tyctx, &method_instance, tycache));
            res
        }
        Rvalue::Cast(CastKind::PointerFromExposedAddress, operand, _) => {
            //FIXME: the documentation of this cast(https://doc.rust-lang.org/nightly/std/ptr/fn.from_exposed_addr.html) is a bit confusing,
            //since this seems to be something deeply linked to the rust memory model.
            // I assume this to be ALWAYS equivalent to `usize as *const/mut T`, but this may not always be the case.
            // If something breaks in the fututre, this is a place that needs checking.

            // Cast from usize/isize to any *T is a NOP, so we just have to load the operand.
            handle_operand(operand, tyctx, method, method_instance, tycache)
        }
        Rvalue::Cast(CastKind::PointerExposeAddress, operand, _) => {
            //FIXME: the documentation of this cast(https://doc.rust-lang.org/nightly/std/primitive.pointer.html#method.expose_addrl) is a bit confusing,
            //since this seems to be something deeply linked to the rust memory model.
            // I assume this to be ALWAYS equivalent to `*const/mut T as usize`, but this may not always be the case.
            // If something breaks in the fututre, this is a place that needs checking.

            // Cast to usize/isize from any *T is a NOP, so we just have to load the operand.
            handle_operand(operand, tyctx, method, method_instance, tycache)
        }
        Rvalue::Cast(CastKind::FloatToFloat, operand, target) => {
            let target = crate::utilis::monomorphize(&method_instance, *target, tyctx);
            let target = tycache.type_from_cache(target, tyctx, Some(method_instance));
            let mut ops = handle_operand(operand, tyctx, method, method_instance, tycache);
            match target {
                Type::F32 => ops.push(CILOp::ConvF32(false)),
                Type::F64 => ops.push(CILOp::ConvF64(false)),
                _ => panic!("Can't preform a FloatToFloat cast to type {target:?}"),
            }
            ops
        }
        Rvalue::Cast(kind, _operand, _) => todo!("Unhandled cast kind {kind:?}, rvalue:{rvalue:?}"),
        Rvalue::Discriminant(place) => {
            let mut ops =
                crate::place::place_adress(place, tyctx, method, method_instance, tycache);
            let owner_ty = place.ty(method, tyctx).ty;
            let owner_ty = crate::utilis::monomorphize(&method_instance, owner_ty, tyctx);
            let owner = tycache.type_from_cache(owner_ty, tyctx, Some(method_instance));
            //TODO: chose proper tag type based on variant count of `owner`
            let discr_type = crate::r#type::Type::U8; //owner_ty
            let owner = if let crate::r#type::Type::DotnetType(dotnet_type) = owner {
                dotnet_type.as_ref().clone()
            } else {
                panic!();
            };
            ops.push(CILOp::LDField(Box::new(crate::cil::FieldDescriptor::new(
                owner,
                discr_type,
                "_tag".into(),
            ))));
            ops
        }
        Rvalue::Len(operand) => {
            let mut ops =
                crate::place::place_adress(operand, tyctx, method, method_instance, tycache);
            let ty = operand.ty(method, tyctx);
            let ty = crate::utilis::monomorphize(&method_instance, ty, tyctx);
            // let tpe = tycache.type_from_cache(ty.ty, tyctx, Some(method_instance));
            match ty.ty.kind() {
                TyKind::Slice(inner) => {
                    let slice_tpe = tycache
                        .slice_ty(*inner, tyctx, Some(method_instance))
                        .as_dotnet()
                        .unwrap();
                    let descriptor =
                        FieldDescriptor::new(slice_tpe, Type::USize, "metadata".into());
                    ops.extend([CILOp::LDField(descriptor.into())]);
                }
                _ => todo!("Get length of type {ty:?}"),
            }
            ops
        }
        Rvalue::Repeat(operand, times) => {
            let times = times
                .try_eval_target_usize(tyctx, ParamEnv::reveal_all())
                .expect("Could not evalute array size as usize.");
            let array =
                crate::utilis::monomorphize(&method_instance, rvalue.ty(method, tyctx), tyctx);
            let array = tycache.type_from_cache(array, tyctx, Some(method_instance));
            let array_dotnet = array.clone().as_dotnet().expect("Invalid array type.");

            let operand_type = tycache.type_from_cache(
                crate::utilis::monomorphize(&method_instance, operand.ty(method, tyctx), tyctx),
                tyctx,
                Some(method_instance),
            );
            let operand = handle_operand(operand, tyctx, method, method_instance, tycache);
            let mut ops = Vec::new();
            ops.push(CILOp::NewTMPLocal(array.clone().into()));
            for idx in 0..times {
                ops.push(CILOp::LoadAddresOfTMPLocal);
                ops.extend(operand.clone());
                ops.extend([CILOp::LdcI64(idx as u64 as i64), CILOp::ConvUSize(false)]);
                ops.push(CILOp::Call(
                    CallSite::new(
                        Some(array_dotnet.clone()),
                        "set_Item".into(),
                        FnSig::new(
                            &[array.clone(), Type::USize, operand_type.clone()],
                            &Type::Void,
                        ),
                        false,
                    )
                    .into(),
                ));
            }
            ops.push(CILOp::LoadTMPLocal);
            ops.push(CILOp::FreeTMPLocal);
            ops
        }
        _ => rustc_middle::ty::print::with_no_trimmed_paths! {todo!("Unhandled RValue {rvalue:?}")},
    };
    res
}
fn align_of<'tcx>(ty: rustc_middle::ty::Ty<'tcx>, tyctx: TyCtxt<'tcx>) -> u64 {
    let layout = tyctx
        .layout_of(rustc_middle::ty::ParamEnvAnd {
            param_env: ParamEnv::reveal_all(),
            value: ty,
        })
        .expect("Can't get layout of a type.")
        .layout;

    let align = layout.align.abi;
    // FIXME: this field is likely private for a reason. I should not do this get its value. Find a better way to get aligement.
    let pow2 = u64::from(unsafe { std::mem::transmute::<_, u8>(align) });
    1 << pow2
}
