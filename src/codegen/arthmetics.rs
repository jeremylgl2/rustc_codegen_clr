use crate::{base_ir::BaseIR, types::Type};
use rustc_middle::mir::{BinOp, UnOp};

use super::sizeof_ops;
pub(crate) fn add_unchecked<'ctx>(a: Type, b: Type) -> BaseIR {
    match (a, b) {
        (Type::I128 | Type::U128, _) => todo!("Can't add 128 bit numbers yet!"),
        (_, Type::I128 | Type::U128) => todo!("Can't add 128 bit numbers yet!"),
        _ => BaseIR::Add,
    }
}
pub(crate) fn binop_unchecked<'ctx>(
    binop: BinOp,
    a: (Vec<BaseIR>, Type),
    b: (Vec<BaseIR>, Type),
) -> Vec<BaseIR> {
    let mut ops = Vec::new();
    //ops.extend(a.0);
    //ops.extend(b.0);
    match binop {
        BinOp::Add | BinOp::AddUnchecked => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(add_unchecked(a.1, b.1));
        }
        BinOp::Sub | BinOp::SubUnchecked => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Sub);
        }
        BinOp::Mul | BinOp::MulUnchecked => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Mul);
        }
        BinOp::Shl | BinOp::ShlUnchecked => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Shl)
        }
        BinOp::Shr | BinOp::ShrUnchecked => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Shr)
        }
        BinOp::Eq => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Eq)
        }
        BinOp::Ne => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.extend([BaseIR::Eq, BaseIR::LDConstI32(0), BaseIR::Eq])
        }
        BinOp::Gt => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Gt)
        }
        BinOp::Lt => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Lt)
        }
        BinOp::Ge => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Lt);
            ops.push(BaseIR::LDConstI32(0));
            ops.push(BaseIR::Eq);
        }
        BinOp::Le => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Gt);
            ops.push(BaseIR::LDConstI32(0));
            ops.push(BaseIR::Eq);
        }
        BinOp::Rem => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Rem)
        }
        BinOp::BitXor => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Xor)
        }
        BinOp::BitOr => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Or)
        }
        BinOp::BitAnd => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::And)
        }
        BinOp::Div => {
            ops.extend(a.0);
            ops.extend(b.0);
            ops.push(BaseIR::Div)
        }
        BinOp::Offset => {
            ops.extend(b.0);
            ops.push(BaseIR::SizeOf(Box::new(
                a.1.pointed_type()
                    .expect("Tried to get offset of non-pointer type!")
                    .clone(),
            )));
            ops.push(BaseIR::Mul);
            ops.extend(a.0);
            ops.push(BaseIR::Add);
        } //todo!("Can't yet handle the pointer offset operator!"),
    };
    ops
}
pub(crate) fn unop_unchecked<'ctx>(unop: UnOp, operand: (Vec<BaseIR>, Type)) -> Vec<BaseIR> {
    match unop {
        UnOp::Not => {
            let mut ops = operand.0;
            ops.push(BaseIR::Not);
            ops
        }
        UnOp::Neg => match operand.1 {
            Type::I8
            | Type::U8
            | Type::I16
            | Type::U16
            | Type::I32
            | Type::U32
            | Type::I64
            | Type::U64
            | Type::ISize
            | Type::USize => {
                let mut ops = vec![BaseIR::LDConstI32(0), BaseIR::Sub];
                ops.extend(operand.0);
                ops
            }
            Type::F32 | Type::F64 => {
                let mut ops = vec![BaseIR::LDConstF32(0.0), BaseIR::Sub];
                ops.extend(operand.0);
                ops
            }
            Type::I128 | Type::U128 => todo!("Can't negate 128 bit intigers"),
            _ => panic!("Negating \"{:?}\" is invalid", operand.1),
        },
    }
}
