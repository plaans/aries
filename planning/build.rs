/*
 *   Copyright (c) 2021
 *   All rights reserved.
 */
// Copyright 2021 Franklin Selva. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

extern crate protoc_rust;

use protoc_rust::{Codegen, Customize};

fn main() {
    Codegen::new()
        .out_dir("src/upf")
        .include("src/upf")
        .customize(Customize {
            ..Default::default()
        })
        .input("src/upf/upf.proto")
        .include("src/upf")
        .run()
        .expect("protoc");
}
