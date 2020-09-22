use std::fs::File;
use crate::classical::state::*;
use crate::classical::GroundProblem;
use crate::symbols::{SymbolTable};
use std::io::Write;


pub fn writeplan(path: String,plan: &[Op],ground: &GroundProblem,symb: &SymbolTable<String, String>){
    // let path = "graphique.dot";

    //let path = path.split_whitespace();
    let mut copypath=path;
    copypath.pop();
     let mut output = File::create(copypath).expect("Something went wrong reading the file");
 
     for &op in plan {
         writeln!(output, "{}",symb.format(ground.operators.name(op)))
                         .expect("Something went wrong writing the file");
     }
 
 }
 