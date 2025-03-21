from __future__ import annotations
import argparse
from pathlib import Path
import re

TAB = " " * 4

def upper_camel(s: str) -> str:
    words = s.split("_")
    return "".join(map(lambda w: w.capitalize(), words))

def snake(s: str) -> str:
    return re.sub(
        "[A-Z]",
        lambda m: "_" + m.group(0).lower(),
        s
    ).removeprefix("_")

def fix_as_bs(s: str) -> str:
    if len(s) == 2:
        s = s[0]
    return s


class ParsingError(Exception):
    pass


class Identifier:
    PATTERN = re.compile("[A-Za-z][A-Za-z0-9_]*")

    @classmethod
    def check(cls, s: str) -> str:
        if not cls.PATTERN.fullmatch(s):
            raise ParsingError(f"'{s}' is not a valid identifier")
        return s


class ArgType:

    def __init__(self, flatzinc_type: str, rust_type: str, rust_import: str, rust_expr_fn: str) -> None:
        self.flatzinc_type = flatzinc_type
        self.rust_type = rust_type
        self.rust_import = rust_import
        self.rust_expr_fn = rust_expr_fn
        self.need_rc = "Rc" in rust_type
        self.is_array = "Vec" in rust_type

    def from_str(self, s: str) -> ArgType:
        if s == self.flatzinc_type:
            return self
        raise ParsingError(f"'{s}' is not valid for {self}")

    def __str__(self) -> str:
        return self.flatzinc_type


class ArgTypeFactory:

    def __init__(self, arg_types: list[ArgType]) -> None:
        self.arg_types = arg_types

    def from_str(self, s: str) -> ArgType:
        for arg_type in self.arg_types:
            try:
                return arg_type.from_str(s)
            except ParsingError:
                continue
        raise ParsingError(f"'{s}' is not a valid arg type")


class Arg:
    ARG_TYPE_FACTORY = ArgTypeFactory(
        [
            ArgType(
                flatzinc_type="int",
                rust_type="Int",
                rust_import="use crate::fzn::types::Int;\n",
                rust_expr_fn="int_from_expr",
            ),
            ArgType(
                flatzinc_type="var int",
                rust_type="Rc<VarInt>",
                rust_import="use crate::fzn::var::VarInt;\n",
                rust_expr_fn="var_int_from_expr",
            ),
            ArgType(
                flatzinc_type="array [int] of int",
                rust_type="Vec<Int>",
                rust_import="use crate::fzn::types::Int;\n",
                rust_expr_fn="vec_int_from_expr",
            ),
            ArgType(
                flatzinc_type="array [int] of var int",
                rust_type="Vec<Rc<VarInt>>",
                rust_import="use crate::fzn::var::VarInt;\n",
                rust_expr_fn="vec_var_int_from_expr",
            ),
            ArgType(
                flatzinc_type="bool",
                rust_type="bool",
                rust_import="",
                rust_expr_fn="bool_from_expr",
            ),
            ArgType(
                flatzinc_type="var bool",
                rust_type="Rc<VarBool>",
                rust_import="use crate::fzn::var::VarBool;\n",
                rust_expr_fn="var_bool_from_expr",
            ),
            ArgType(
                flatzinc_type="array [int] of bool",
                rust_type="Vec<bool>",
                rust_import="",
                rust_expr_fn="vec_bool_from_expr",
            ),
            ArgType(
                flatzinc_type="array [int] of var bool",
                rust_type="Vec<Rc<VarBool>>",
                rust_import="use crate::fzn::var::VarBool;\n",
                rust_expr_fn="vec_var_bool_from_expr",
            ),
        ]
    )

    def __init__(self, type_: ArgType, identifier: str) -> None:
        self.type = type_
        self.identifier = fix_as_bs(identifier)

    @classmethod
    def from_str(cls, s: str) -> Arg:
        match s.split(":"):
            case raw_arg_type, raw_identifier:
                raw_arg_type = raw_arg_type.strip()
                raw_identifier = raw_identifier.strip()
                arg_type = cls.ARG_TYPE_FACTORY.from_str(raw_arg_type)
                identifier = Identifier.check(raw_identifier)
                return cls(arg_type, identifier)
        raise ParsingError(f"'{s}' is not a valid arg")

    def __str__(self) -> str:
        return f"{self.type}: {self.identifier}"

    def rust_attr(self) -> str:
        return f"{self.identifier}: {self.type.rust_type}"

    def rust_getter(self) -> str:
        getter = TAB + f"pub fn {self.identifier}(&self) -> &{self.type.rust_type} {{\n"
        getter += 2*TAB + f"&self.{self.identifier}\n"
        getter += TAB + "}\n"
        return getter


class Predicate:
    PATTERN = re.compile(r"predicate ([^(]+)\((.+)\)")
    DERIVE = "#[derive(Clone, Debug)]\n"

    def __init__(self, identifer: Identifier, args: list[Arg]) -> None:
        self.identifier = Identifier.check(identifer)
        self.rust_name = upper_camel(identifer)
        self.args = args

    @classmethod
    def from_str(cls, s: str) -> Predicate:
        m = Predicate.PATTERN.fullmatch(s)
        if not m:
            raise ParsingError(f"'{s}' is not a valid predicate")
        identifier, raw_args = m.groups()
        raw_args = raw_args.split(",")
        args = [Arg.from_str(raw_arg) for raw_arg in raw_args]
        return cls(identifier, args)

    @classmethod
    def from_file(cls, s: str) -> list[Predicate]:
        predicates = []
        for line in s.splitlines():
            if line.startswith("%"):
                continue
            predicate = Predicate.from_str(line)
            predicates.append(predicate)
        return predicates

    def __str__(self) -> str:
        args = ", ".join(map(str, self.args))
        return f"predicate {self.identifier}({args})"

    def rust_imports(self) -> str:
        imports = "use std::rc::Rc;\n"
        imports += "\n"
        imports += "use flatzinc::ConstraintItem;\n"
        imports += "\n"
        from_expr_fns = set(arg.type.rust_expr_fn for arg in self.args)
        for from_expr_fn in sorted(from_expr_fns):
            imports += f"use crate::fzn::parser::{from_expr_fn};\n"
        imports += "use crate::fzn::constraint::Constraint;\n"
        imports += "use crate::fzn::model::Model;\n"
        imports += "use crate::fzn::Fzn;\n"
        type_imports = set(arg.type.rust_import for arg in self.args)
        for type_import in sorted(type_imports):
            imports += type_import
        return imports

    def rust_struct(self) -> str:
        struct = f"pub struct {self.rust_name} {{\n"
        for arg in self.args:
            attribute = arg.rust_attr()
            struct += TAB + f"{attribute},\n"
        struct += "}\n"
        return struct

    def rust_new(self) -> str:
        new = f"{TAB}pub fn new("
        new += ", ".join(arg.rust_attr() for arg in self.args)
        new += ") -> Self {\n"
        new += 2*TAB + "Self { "
        new += ", ".join(arg.identifier for arg in self.args)
        new += " }\n"
        new += TAB + "}\n"
        return new

    def rust_getters(self) -> str:
        getters = "\n".join(arg.rust_getter() for arg in self.args)
        return getters

    def rust_try_from_item(self) -> str:
        try_from = TAB + "pub fn try_from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<Self> {\n"
        try_from += 2*TAB + "anyhow::ensure!(\n"
        try_from += 3*TAB + "item.id.as_str() == Self::NAME,\n"
        try_from += 3*TAB + "\"'{}' expected but received '{}'\",\n"
        try_from += 3*TAB + "Self::NAME,\n"
        try_from += 3*TAB + "item.id,\n"
        try_from += 2*TAB + ");\n"
        try_from += 2*TAB + "anyhow::ensure!(\n"
        try_from += 3*TAB + "item.exprs.len() == Self::NB_ARGS,\n"
        try_from += 3*TAB + "\"{} args expected but received {}\",\n"
        try_from += 3*TAB + "Self::NB_ARGS,\n"
        try_from += 3*TAB + "item.exprs.len(),\n"
        try_from += 2*TAB + ");\n"
        for i, arg in enumerate(self.args):
            try_from += 2*TAB + f"let {arg.identifier} = {arg.type.rust_expr_fn}(&item.exprs[{i}], model)?;\n"
        try_from += 2*TAB + "Ok(Self::new(" + ", ".join(arg.identifier for arg in self.args) + "))\n"
        try_from += TAB + "}\n"
        return try_from

    def rust_impl(self) -> str:
        impl = f"impl {self.rust_name} {{\n"
        impl += TAB + f'pub const NAME: &str = "{self.identifier}";\n'
        impl += TAB + f'pub const NB_ARGS: usize = {len(self.args)};\n'
        impl += "\n"
        impl += self.rust_new()
        impl += "\n"
        impl += self.rust_getters()
        impl += "\n"
        impl += self.rust_try_from_item()
        impl += "}\n"
        return impl

    def rust_impl_flatzinc(self) -> str:
        fzn = f"impl Fzn for {self.rust_name} {{\n"
        fzn += TAB + "fn fzn(&self) -> String {\n"
        fzn += 2*TAB + 'format!('
        fzn += '"{}(' + ", ".join(["{:?}"]*len(self.args))
        fzn += ');\\n", Self::NAME, '
        fzn += ", ".join(f"self.{arg.identifier}.fzn()" for arg in self.args)
        fzn += ")\n"
        fzn += TAB + "}\n"
        fzn += "}\n"
        return fzn

    def rust_try_from_constraint(self) -> str:
        try_from = f"impl TryFrom<Constraint> for {self.rust_name} {{\n"
        try_from += TAB + "type Error = anyhow::Error;\n"
        try_from += "\n"
        try_from += TAB + "fn try_from(value: Constraint) -> Result<Self, Self::Error> {\n"
        try_from += 2*TAB + "match value {\n"
        try_from += 3*TAB + f"Constraint::{self.rust_name}(c) => Ok(c),\n"
        try_from += 3*TAB + '_ => anyhow::bail!("unable to downcast to {}", Self::NAME),\n'
        try_from += 2*TAB + "}\n"
        try_from += TAB + "}\n"
        try_from += "}\n"
        return try_from

    def rust_from_for_constraint(self) -> str:
        from_for = f"impl From<{self.rust_name}> for Constraint {{\n"
        from_for += TAB + f"fn from(value: {self.rust_name}) -> Self {{\n"
        from_for += 2*TAB + f"Self::{self.rust_name}(value)\n"
        from_for += TAB + "}\n"
        from_for += "}\n"
        return from_for

    def rust_file(self) -> str:
        file = self.rust_imports()
        file += "\n"
        file += self.DERIVE
        file += self.rust_struct()
        file += "\n"
        file += self.rust_impl()
        file += "\n"
        file += self.rust_impl_flatzinc()
        file += "\n"
        file += self.rust_try_from_constraint()
        file += "\n"
        file += self.rust_from_for_constraint()
        return file


class BuiltinsMod:
    def __init__(self, predicates: list[Predicate]) -> None:
        self.predicates = predicates

    def rust_mods(self) -> str:
        mods = ""
        for predicate in self.predicates:
            mods += f"mod {predicate.identifier};\n"
        return mods

    def rust_uses(self) -> str:
        uses = ""
        for predicate in self.predicates:
            uses += f"pub use {predicate.identifier}::{predicate.rust_name};\n"
        return uses

    def rust_file(self) -> str:
        file = self.rust_mods()
        file += "\n"
        file += self.rust_uses()
        return file


class ConstraintMod:
    FILE = (
        "pub mod builtins;\n"
        "mod constraint;\n"
        "\n"
        "pub use constraint::Constraint;\n"
    )


class Constraint:
    DERIVE = "#[derive(Clone, Debug)]\n"

    def __init__(self, predicates: list[Predicate]) -> None:
        self.predicates = predicates

    def rust_imports(self) -> str:
        return "use crate::fzn::constraint::builtins::*;\n"

    def rust_enum(self) -> str:
        enum = "pub enum Constraint {\n"
        for predicate in self.predicates:
            enum += TAB + f"{predicate.rust_name}({predicate.rust_name}),\n"
        enum += "}\n"
        return enum

    def rust_file(self) -> str:
        file = self.rust_imports()
        file += "\n"
        file += self.DERIVE
        file += self.rust_enum()
        return file


def output_dir(s: str) -> Path:
    path = Path(s)
    if not path.exists():
        raise argparse.ArgumentTypeError(f"{s} is not a valid path")
    if not path.is_dir():
        raise argparse.ArgumentTypeError(f"{path} is not a directory")
    if any(path.iterdir()):
        raise argparse.ArgumentTypeError(f"{path} is not empty")
    return path


def write_files(l: list[tuple[Path, str]]) -> None:
    for path, content in l:
        path.write_text(content)


def print_files(l: list[tuple[Path, str]]) -> None:
    for path, content in l:
        print("//", f" {path} ".center(60, "-"))
        print(content)


def main(args: argparse.Namespace) -> None:
    raw_predicates = args.input.read()
    predicates = Predicate.from_file(raw_predicates)
    constraint = Constraint(predicates)
    builtins_mod = BuiltinsMod(predicates)

    constraint_dir: Path = args.output
    builtins_dir: Path = constraint_dir / "builtins"

    if not args.debug:
        builtins_dir.mkdir(exist_ok=True)

    path_content = [
        (builtins_dir / "mod.rs", builtins_mod.rust_file()),
        (constraint_dir / "constraint.rs", constraint.rust_file()),
        (constraint_dir / "mod.rs", ConstraintMod.FILE),
    ]

    for predicate in predicates:
        path = builtins_dir / f"{predicate.identifier}.rs"
        content = predicate.rust_file()
        path_content.append((path, content))

    fn_out = print_files if args.debug else write_files
    fn_out(path_content)

if __name__ == "__main__":

    parser = argparse.ArgumentParser(
        prog="constraints",
        description="Meta programming script to generate builtin constraints.",
    )

    parser.add_argument(
        "-d", "--debug",
        help="print files on stdout",
        action="store_true",
    )

    parser.add_argument(
        "input",
        help="flatzinc predicates",
        type=argparse.FileType("r"),
    )

    parser.add_argument(
        "output",
        help="constraint directory",
        type=output_dir,
    )

    args = parser.parse_args()

    main(args)

