from __future__ import annotations
from abc import ABC
import re
import sys

TAB = " " * 4

def upper_camel(s: str) -> str:
    words = s.split("_")
    return "".join(map(lambda w: w.capitalize(), words))


class ParsingError(Exception):
    pass


class Identifier:
    PATTERN = re.compile("[A-Za-z][A-Za-z0-9_]*")

    def __init__(self, value: str) -> None:
        if not Identifier.PATTERN.fullmatch(value):
            raise ParsingError(f"'{value}' is not a valid identifier")
        self.value = value

    def __str__(self) -> str:
        return self.value


class AbstractArgType(ABC):
    FLATZINC_TYPE: str
    IS_ARRAY: bool

    @classmethod
    def from_str(cls, s: str) -> AbstractArgType:
        if s == cls.FLATZINC_TYPE:
            return cls()
        raise ParsingError(f"'{s}' is not valid for {cls.__name__}")

    def __str__(self) -> str:
        return self.__class__.FLATZINC_TYPE

    @classmethod
    def rust_type(cls) -> str:
        return cls.__name__ # TODO: fix this

    @classmethod
    def rust_boxed_type(cls) -> str:
        basic = f"Rc<{cls.rust_type()}>"
        rust_type = f"Vec<{basic}>" if cls.IS_ARRAY else basic
        return rust_type


class ArgType:
    @classmethod
    def from_str(cls, s: str) -> ArgType:
        for c in cls.__subclasses__():
            try:
                return c.from_str(s)
            except ParsingError:
                continue
        raise ParsingError(f"'{s}' is not a valid arg type")

class ParInt(AbstractArgType, ArgType):
    FLATZINC_TYPE = "int"
    IS_ARRAY = False

class VarInt(AbstractArgType, ArgType):
    FLATZINC_TYPE = "var int"
    IS_ARRAY = False

class ParIntArray(AbstractArgType, ArgType):
    FLATZINC_TYPE = "array [int] of int"
    IS_ARRAY = True

class VarIntArray(AbstractArgType, ArgType):
    FLATZINC_TYPE = "array [int] of var int"
    IS_ARRAY = True

class ParBool(AbstractArgType, ArgType):
    FLATZINC_TYPE = "bool"
    IS_ARRAY = False

class VarBool(AbstractArgType, ArgType):
    FLATZINC_TYPE = "var bool"
    IS_ARRAY = False

class ParBoolArray(AbstractArgType, ArgType):
    FLATZINC_TYPE = "array [int] of bool"
    IS_ARRAY = True

class VarBoolArray(AbstractArgType, ArgType):
    FLATZINC_TYPE = "array [int] of var bool"
    IS_ARRAY = True


class Arg:
    def __init__(self, type_: ArgType, identifier: Identifier) -> None:
        self.type = type_
        self.identifier = identifier

    @classmethod
    def from_str(cls, s: str) -> Arg:
        match s.split(":"):
            case raw_arg_type, raw_identifier:
                raw_arg_type = raw_arg_type.strip()
                raw_identifier = raw_identifier.strip()
                arg_type = ArgType.from_str(raw_arg_type)
                identifier = Identifier(raw_identifier)
                return cls(arg_type, identifier)
        raise ParsingError(f"'{s}' is not a valid arg")

    def __str__(self) -> str:
        return f"{self.type}: {self.identifier}"

    def rust_attr(self) -> str:
        return f"{self.identifier.value}: {self.type.rust_boxed_type()}"

    def rust_getter(self) -> str:
        getter = TAB + f"pub fn {self.identifier}(&self) -> &{self.type.rust_boxed_type()}) {{\n"
        getter += 2*TAB + f"&self.{self.identifier}\n"
        getter += TAB + "}\n"
        return getter


class Predicate:
    PATTERN = re.compile(r"predicate ([^(]+)\((.+)\)")

    def __init__(self, identifer: Identifier, args: list[Arg]) -> None:
        self.identifier = identifer
        self.rust_name = upper_camel(identifer.value)
        self.args = args

    @classmethod
    def from_str(cls, s: str) -> Predicate:
        m = Predicate.PATTERN.fullmatch(s)
        if not m:
            raise ParsingError(f"'{s}' is not a valid predicate")
        raw_identifier, raw_args = m.groups()
        raw_args = raw_args.split(",")
        args = [Arg.from_str(raw_arg) for raw_arg in raw_args]
        identifier = Identifier(raw_identifier)
        return cls(identifier, args)

    def __str__(self) -> str:
        args = ", ".join(map(str, self.args))
        return f"predicate {self.identifier}({args})"

    def rust_struct(self) -> str:
        struct = f"pub struct {self.rust_name} {{\n"
        for arg in self.args:
            attribute = arg.rust_attr()
            struct += f"{TAB}{attribute},\n"
        struct += "}\n"
        return struct

    def rust_new(self) -> str:
        new = f"{TAB}pub fn new("
        new += ", ".join(arg.rust_attr() for arg in self.args)
        new += ") {\n"
        new += 2*TAB + "Self { "
        new += ", ".join(arg.identifier.value for arg in self.args)
        new += " }\n"
        new += TAB + "}\n"
        return new

    def rust_getters(self) -> str:
        getters = "\n".join(arg.rust_getter() for arg in self.args)
        return getters

    def rust_impl(self) -> str:
        impl = f"impl {self.rust_name} {{\n"
        impl += TAB + f'pub const NAME: &str = "{self.identifier}";\n'
        impl += "\n"
        impl += self.rust_new()
        impl += "\n"
        impl += self.rust_getters()
        impl += "}\n"
        return impl

# use std::rc::Rc;

# use anyhow::bail;

# use crate::constraint::Constraint;
# use crate::var::VarBool;

    def rust_imports(self) -> str:
        imports = "use std::rc::Rc;\n"
        imports += "\n"
        imports += "use crate::constraint::Constraint;\n"
        arg_types = set(arg.type.rust_type() for arg in self.args)
        for arg_type in arg_types:
            imports += f"use crate::var::{arg_type};\n"
        return imports

    def rust_try_from_constraint(self) -> str:
        try_from = f"impl TryFrom<Constraint> for {self.rust_name} {{\n"
        try_from += 2*TAB + "type Error = anyhow::Error;\n"
        try_from += "\n"
        try_from += 2*TAB + "fn try_from(value: Constraint) -> Result<Self, Self::Error> {\n"
        try_from += 2*TAB + "match value {\n"
        try_from += 3*TAB + f"Constraint::{self.rust_name}(c) => Ok(c),\n"
        try_from += 3*TAB + '_ => anyhow::bail!("unable to downcast to {}", Self.NAME),\n'
        try_from += 2*TAB + "}\n"
        try_from += TAB + "}\n"
        try_from += "}\n"
        return try_from

    def rust_file(self) -> str:
        file = self.rust_imports()
        file += "\n"
        file += self.rust_struct()
        file += "\n"
        file += self.rust_impl()
        file += "\n"
        file += self.rust_try_from_constraint()
        return file


if __name__ == "__main__":
    lines = sys.stdin.read().splitlines()
    for line in lines:
        if line.startswith("%"):
            continue
        pred = Predicate.from_str(line)
        print("-"*20, pred.identifier, "-"*20)
        print(pred.rust_file())
