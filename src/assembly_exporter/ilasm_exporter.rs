use super::{ilasm_op::dotnet_type_ref_cli, AssemblyExporter};
use crate::{
    access_modifier::AccessModifer,
    assembly_exporter::{
        ilasm_op::{non_void_type_cil, type_cil},
        AssemblyExportError,
    },
    method::Method,
    r#type::TypeDef,
    r#type::{DotnetTypeRef, Type},
};
use std::{borrow::Cow, io::Write};
#[must_use]
/// A struct used to export an asssembly using the ILASM tool as a .NET assembly creator.
pub struct ILASMExporter {
    encoded_asm: Vec<u8>,
}
impl std::io::Write for ILASMExporter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.encoded_asm.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.encoded_asm.flush()
    }
}
impl AssemblyExporter for ILASMExporter {
    fn add_global(&mut self, tpe: &Type, name: &str) {
        writeln!(
            self,
            ".field static {tpe} {name}",
            tpe = non_void_type_cil(tpe)
        )
        .expect("Could not write global!")
    }
    fn init(asm_name: &str) -> Self {
        let mut encoded_asm = Vec::with_capacity(0x1_00);
        write!(encoded_asm, ".assembly {asm_name}{{}}").expect("Write error!");
        Self { encoded_asm }
    }
    fn add_extern_ref(
        &mut self,
        asm_name: &str,
        asm_ref_data: &crate::assembly::AssemblyExternRef,
    ) {
        let (v1, v2, v3, v4) = asm_ref_data.version();
        write!(
            self.encoded_asm,
            ".assembly extern {asm_name}{{.ver {v1}:{v2}:{v3}:{v4} }}"
        )
        .expect("Write error!");
    }
    fn add_type(&mut self, tpe: &TypeDef) {
        type_def_cli(&mut self.encoded_asm, tpe, false).expect("Error");
        //let _ = self.types.push(tpe.clone());
    }
    fn add_method(&mut self, method: &Method) {
        method_cil(&mut self.encoded_asm, method).expect("Error");
    }
    fn finalize(
        self,
        final_path: &std::path::Path,
        is_dll: bool,
    ) -> Result<(), AssemblyExportError> {
        let directory = absolute_path(final_path)
            .map_err(|io| AssemblyExportError::CouldNotCanonalizePath(io, final_path.to_owned()))?
            .parent()
            .expect("Can't get the target directory")
            .to_owned();

        let mut out_path = directory.clone();
        out_path.set_file_name(final_path.file_name().expect("Target file has no name!"));
        if let Some(ext) = final_path.extension() {
            out_path = out_path.with_extension(ext);
        }
        //final_path.expect("Could not canonialize path!");

        let cil_path = out_path.with_extension("il");
        let cil = self.encoded_asm;
        std::fs::File::create(&cil_path)
            .expect("Could not create file")
            .write_all(&cil)
            .expect("Could not write bytes");
        let asm_type = if is_dll { "-dll" } else { "-exe" };
        let target = format!(
            "-output:{out_path}",
            out_path = out_path.clone().to_string_lossy()
        );
        let args: [String; 3] = [
            asm_type.into(),
            target,
            cil_path.clone().to_string_lossy().to_string(),
        ];
        let out = std::process::Command::new("ilasm")
            .args(args)
            .output()
            .expect("failed run ilasm process");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !stdout.contains("\nOperation completed successfully\n") {
            let err = format!(
                "stdout:{} stderr:{}",
                stdout,
                String::from_utf8_lossy(&out.stderr)
            );
            return Err(AssemblyExportError::ExporterError(err.into()));
        }
        Ok(())
    }
}
fn type_def_cli(
    w: &mut impl Write,
    tpe: &TypeDef,
    is_nested: bool,
) -> Result<(), super::AssemblyExportError> {
    let name = tpe.name();
    assert!(
        tpe.gargc() == 0,
        "Generic typedefs not supported yet. tpe:{tpe:?}"
    );
    let extends = if let Some(extended) = tpe.extends() {
        todo!("Can't handle inheretence yet. Typedef inherits from {extended:?}!");
    } else {
        "[System.Runtime]System.ValueType"
    };
    let access = if let AccessModifer::Public = tpe.access_modifier() {
        "public"
    } else {
        "private"
    };
    let sealed = if tpe.explicit_offsets().is_some() || tpe.extends().is_none() {
        "sealed"
    } else {
        ""
    };
    let explicit = if tpe.explicit_offsets().is_some() {
        "explicit"
    } else {
        ""
    };
    let nested = if is_nested { "nested" } else { "" };
    writeln!(w,".class {nested} {access} {explicit} ansi {sealed} beforefieldinit {name} extends {extends}{{")?;
    for inner_type in tpe.inner_types() {
        type_def_cli(w, inner_type, true)?;
    }
    if let Some(offsets) = tpe.explicit_offsets() {
        for ((field_name, field_type), offset) in tpe.fields().iter().zip(offsets.iter()) {
            writeln!(
                w,
                "\t.field [{offset}] public {field_type_name} {field_name}",
                field_type_name = non_void_type_cil(field_type)
            )?;
        }
    } else {
        for (field_name, field_type) in tpe.fields() {
            writeln!(
                w,
                "\t.field public {field_type_name} {field_name}",
                field_type_name = non_void_type_cil(field_type)
            )?;
        }
    }
    for method in tpe.methods() {
        method_cil(w, method)?;
    }
    writeln!(w, "}}")?;
    Ok(())
}
fn method_cil(w: &mut impl Write, method: &Method) -> std::io::Result<()> {
    let access = if let AccessModifer::Private = method.access() {
        "private"
    } else {
        "public"
    };
    let static_inst = if method.is_static() {
        "static"
    } else {
        "instance"
    };
    let output = type_cil(method.sig().output());
    let name = method.name();
    write!(
        w,
        ".method {access} hidebysig {static_inst} {output} {name}("
    )?;
    let mut input_iter = method.explicit_inputs().iter();
    if let Some(input) = input_iter.next() {
        write!(w, "{}", non_void_type_cil(input))?;
    }
    for input in input_iter {
        write!(w, ",{}", non_void_type_cil(input))?;
    }
    writeln!(w, "){{")?;
    if method.is_entrypoint() {
        writeln!(w, ".entrypoint")?;
    }
    if crate::ALWAYS_INIT_LOCALS {
        writeln!(w, "\t.locals init(")?;
    } else {
        writeln!(w, "\t.locals (")?;
    }
    let mut locals_iter = method.locals().iter().enumerate();
    if let Some((local_id, local)) = locals_iter.next() {
        write!(
            w,
            "\t\t[{local_id}] {escaped_type}",
            escaped_type = non_void_type_cil(&local.1)
        )?;
    }
    for (local_id, local) in locals_iter {
        write!(
            w,
            ",\n\t\t[{local_id}] {escaped_type}",
            escaped_type = non_void_type_cil(&local.1)
        )?;
    }
    writeln!(w, "\n\t)")?;
    for op in method.get_ops() {
        writeln!(w, "\t{op_cli}", op_cli = super::ilasm_op::op_cli(op))?;
    }
    writeln!(w, "}}")
}
fn absolute_path(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    if path.has_root() {
        Ok(path.to_owned())
    } else {
        let mut abs_path = std::env::current_dir()?;
        abs_path.extend(path);
        Ok(abs_path)
    }
}
