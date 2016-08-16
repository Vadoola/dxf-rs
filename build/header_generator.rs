// Copyright (c) IxMilia.  All Rights Reserved.  Licensed under the Apache License, Version 2.0.  See License.txt in the project root for license information.

extern crate xml;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Write};
use self::xml::reader::{EventReader, XmlEvent};

include!("../src/expected_type.rs");

pub fn generate_header() {
    let variables = gather_variables();
    let mut fun = String::new();
    fun.push_str("
// The contents of this file are automatically generated and should not be modified directly.  See the `src/build` directory.

// types from `lib.rs`.
use ::{DxfCodePair, DxfCodePairAsciiWriter, DxfColor, DxfLineWeight, DxfPoint, DxfVector};
use ::helper_functions::*;

use enums::*;
use enum_primitive::FromPrimitive;
use std::io;
use std::io::Write;

extern crate chrono;
use self::chrono::{DateTime, Local, UTC};

extern crate time;
use self::time::Duration;

extern crate uuid;
use self::uuid::Uuid;

// Used to turn Option<T> into io::Result.
macro_rules! try_result {
    ($expr : expr) => (
        match $expr {
            Some(v) => v,
            None => return Err(io::Error::new(io::ErrorKind::InvalidData, \"unexpected enum value\"))
        }
    )
}
".trim_left());
    generate_struct(&mut fun, &variables);

    fun.push_str("impl DxfHeader {\n");
    generate_new(&mut fun, &variables);
    generate_flags(&mut fun, &variables);
    generate_set_defaults(&mut fun, &variables);
    generate_set_header_value(&mut fun, &variables);
    generate_add_code_pairs(&mut fun, &variables);
    fun.push_str("}\n");

    let mut file = File::create("src/header_generated.rs").ok().unwrap();
    file.write_all(fun.as_bytes()).ok().unwrap();
}

fn generate_struct(fun: &mut String, variables: &Vec<HeaderVariable>) {
    let mut seen_fields = HashSet::new();
    fun.push_str("pub struct DxfHeader {\n");
    for v in variables {
        if !seen_fields.contains(&v.field) {
            seen_fields.insert(&v.field);
            fun.push_str(format!("    pub {field}: {typ}, // ${name}\n", field=v.field, typ=v.typ, name=v.name).as_str());
        }
    }

    fun.push_str("}\n");
    fun.push_str("\n");
}

fn generate_new(fun: &mut String, variables: &Vec<HeaderVariable>) {
    let mut seen_fields = HashSet::new();
    fun.push_str("    pub fn new() -> DxfHeader {\n");
    fun.push_str("        DxfHeader {\n");
    for v in variables {
        if !seen_fields.contains(&v.field) {
            seen_fields.insert(&v.field);
            fun.push_str(format!("            {field}: {default_value}, // ${name}\n", field=v.field, default_value=v.default_value, name=v.name).as_str());
        }
    }

    fun.push_str("        }\n");
    fun.push_str("    }\n");
}

fn generate_flags(fun: &mut String, variables: &Vec<HeaderVariable>) {
    let mut seen_fields = HashSet::new();
    for v in variables {
        if !seen_fields.contains(&v.field) {
            seen_fields.insert(&v.field);
            if v.flags.len() > 0 {
                fun.push_str(format!("    // {} flags\n", v.field).as_str());
            }
            for f in &v.flags {
                fun.push_str(format!("    pub fn get_{flag}(&self) -> bool {{\n", flag=f.name).as_str());
                fun.push_str(format!("        self.{field} & {mask} != 0\n", field=v.field, mask=f.mask).as_str());
                fun.push_str("    }\n");
                fun.push_str(format!("    pub fn set_{flag}(&mut self, val: bool) {{\n", flag=f.name).as_str());
                fun.push_str(format!("        if val {{\n").as_str());
                fun.push_str(format!("            self.{field} |= {mask};\n", field=v.field, mask=f.mask).as_str());
                fun.push_str("        }\n");
                fun.push_str("        else {\n");
                fun.push_str(format!("            self.{field} &= !{mask};\n", field=v.field, mask=f.mask).as_str());
                fun.push_str("        }\n");
                fun.push_str("    }\n");
            }
        }
    }
}

fn generate_set_defaults(fun: &mut String, variables: &Vec<HeaderVariable>) {
    let mut seen_fields = HashSet::new();
    fun.push_str("    pub fn set_defaults(&mut self) {\n");
    for v in variables {
        if !seen_fields.contains(&v.field) {
            seen_fields.insert(&v.field);
            fun.push_str(format!("        self.{field} = {default_value}; // ${name}\n", field=v.field, default_value=v.default_value, name=v.name).as_str());
        }
    }

    fun.push_str("    }\n");
}

fn generate_set_header_value(fun: &mut String, variables: &Vec<HeaderVariable>) {
    let mut seen_fields = HashSet::new();
    fun.push_str("    pub fn set_header_value(&mut self, variable: &str, pair: &DxfCodePair) -> io::Result<()> {\n");
    fun.push_str("        match variable {\n");
    for v in variables {
        if !seen_fields.contains(&v.field) { // TODO: handle duplicates
            seen_fields.insert(&v.field);
            fun.push_str(format!("            \"${name}\" => {{ ", name=v.name).as_str());
            if v.code < 0 {
                fun.push_str(format!("try!(self.{field}.set(&pair));", field=v.field).as_str());
            }
            else {
                let expected_type = get_expected_type(v.code).ok().unwrap();
                let reader_fun = get_reader_function(&expected_type);
                let converter = if v.read_converter.is_empty() { "{}" } else { v.read_converter.as_str() };
                let read_cmd = converter.replace("{}", format!("{}(&pair.value)", reader_fun).as_str());
                fun.push_str(format!("try!(verify_code({code}, pair.code)); self.{field} = {cmd};", code=v.code, field=v.field, cmd=read_cmd).as_str());
            }

            fun.push_str(" },\n");
        }
    }
    fun.push_str("            _ => (),\n");
    fun.push_str("        }\n");
    fun.push_str("\n");
    fun.push_str("        Ok(())\n");
    fun.push_str("    }\n");
}

fn generate_add_code_pairs(fun: &mut String, variables: &Vec<HeaderVariable>) {
    fun.push_str("    pub fn write_code_pairs<T>(&self, writer: &mut DxfCodePairAsciiWriter<T>) -> io::Result<()> where T: Write {\n");
    for v in variables {
        // prepare writing predicate
        let mut parts = vec![];
        if !v.min_version.is_empty() {
            parts.push(format!("self.version >= DxfAcadVersion::{}", v.min_version));
        }
        if !v.max_version.is_empty() {
            parts.push(format!("self.version <= DxfAcadVersion::{}", v.max_version));
        }
        if v.dont_write_default {
            parts.push(format!("self.{} != {}", v.field, v.default_value));
        }
        let indent = match parts.len() {
            0 => "",
            _ => "    ",
        };

        // write the value
        fun.push_str(format!("        // ${}\n", v.name).as_str());
        if parts.len() > 0 {
            fun.push_str(format!("        if {} {{\n", parts.join(" && ")).as_str());
        }
        fun.push_str(format!("        {indent}try!(writer.write_code_pair(&DxfCodePair::new_str(9, \"${name}\")));\n", name=v.name, indent=indent).as_str());
        let write_converter = if v.write_converter.is_empty() { "{}" } else { v.write_converter.as_str() };
        if v.code > 0 {
            let expected_type = get_code_pair_type(get_expected_type(v.code).ok().unwrap());
            let value = write_converter.replace("{}", format!("self.{}", v.field).as_str());
            fun.push_str(format!("        {indent}try!(writer.write_code_pair(&DxfCodePair::new_{typ}({code}, {value})));\n",
                code=v.code,
                value=value,
                typ=expected_type,
                indent=indent).as_str());
        }
        else {
            // write a point or vector as it's components
            for i in 0..v.code.abs() {
                let (code, field) = match i {
                    0 => (10, "x"),
                    1 => (20, "y"),
                    2 => (30, "z"),
                    _ => panic!("unexpected number of values"),
                };
                let value = write_converter.replace("{}", format!("self.{}.{}", v.field, field).as_str());
                fun.push_str(format!("        {indent}try!(writer.write_code_pair(&DxfCodePair::new_double({code}, {value})));\n",
                    code=code,
                    value=value,
                    indent=indent).as_str());
            }
        }
        if parts.len() > 0 {
            fun.push_str("        }\n");
        }

        // newline between values
        fun.push_str("\n");
    }

    fun.push_str("        Ok(())\n");
    fun.push_str("    }\n");
}

fn gather_variables() -> Vec<HeaderVariable> {
    let file = File::open("spec/HeaderVariablesSpec.xml").unwrap();
    let file = BufReader::new(file);
    let parser = EventReader::new(file);
    let mut header_variables: Vec<HeaderVariable> = vec![];
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                match name.local_name.as_str() {
                    "Variable" => {
                        let mut var = HeaderVariable::new();
                        for attr in attributes {
                            match attr.name.local_name.as_str() {
                                "Name" => var.name = attr.value,
                                "Code" => var.code = attr.value.parse::<i32>().unwrap(),
                                "Type" => var.typ = attr.value,
                                "Field" => var.field = attr.value,
                                "DefaultValue" => var.default_value = attr.value,
                                "ReadConverter" => var.read_converter = attr.value,
                                "WriteConverter" => var.write_converter = attr.value,
                                "Comment" => var.comment = attr.value,
                                "MinVersion" => var.min_version = attr.value,
                                "MaxVersion" => var.max_version = attr.value,
                                "SuppressWriting" => var.suppress_writing = attr.value == "true",
                                "DontWriteDefault" => var.dont_write_default = attr.value == "true",
                                _ => panic!("unexpected attribute '{}' on Variable element", attr.name),
                            }
                        }

                        header_variables.push(var);
                    },
                    "Flag" => {
                        let mut flag = HeaderVariableFlag::new();
                        for attr in attributes {
                            match attr.name.local_name.as_str() {
                                "Name" => flag.name = attr.value,
                                "Mask" => flag.mask = attr.value.parse::<i32>().unwrap(),
                                "Comment" => flag.comment = attr.value,
                                _ => panic!("unexpected attribute '{}' on Flag element", attr.name),
                            }
                        }

                        let len = header_variables.len();
                        header_variables[len - 1].flags.push(flag);
                    },
                    "Spec" => (),
                    _ => panic!("unexpected start element: {}", name)
                }
            },
            Ok(XmlEvent::EndElement { name: _ }) => {

            },
            Err(e) => {
                panic!("unable to read xml: {}", e);
            }
            _ => (),
        }
    }

    header_variables
}

struct HeaderVariable {
    name: String,
    code: i32,
    typ: String,
    field: String,
    default_value: String,
    read_converter: String,
    write_converter: String,
    comment: String,
    min_version: String,
    max_version: String,
    suppress_writing: bool,
    dont_write_default: bool,
    flags: Vec<HeaderVariableFlag>,
}

impl HeaderVariable {
    pub fn new() -> HeaderVariable {
        HeaderVariable {
            name: String::new(),
            code: 0,
            typ: String::new(),
            field: String::new(),
            default_value: String::new(),
            read_converter: String::new(),
            write_converter: String::new(),
            comment: String::new(),
            min_version: String::new(),
            max_version: String::new(),
            suppress_writing: false,
            dont_write_default: false,
            flags: vec![],
        }
    }
}

struct HeaderVariableFlag {
    name: String,
    mask: i32,
    comment: String,
}

impl HeaderVariableFlag {
    pub fn new() -> HeaderVariableFlag {
        HeaderVariableFlag {
            name: String::new(),
            mask: 0,
            comment: String::new(),
        }
    }
}
