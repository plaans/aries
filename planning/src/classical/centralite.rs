use crate::classical::heuristics::*;
use crate::classical::state::*;
use crate::classical::{GroundProblem};
use crate::classical::state2::*;
use std::fmt::Display;
use nalgebra::base::*;

pub fn calculcentraliteglobal(support : &DMatrix<i32>)->Vec<f32>{
    
    let mut out=Vec::new();
    let i: usize = support.nrows();
    let j: usize = support.ncols();
   // vec![0; i];
    let mut sumligne=vec![0;i];
    let mut sumcolonne=vec![0; j];
    for row in 0..i{
        for col in 0..j{
            if support[(row,col)]!=0{
                sumligne[row]=sumligne[row]+1;
                sumcolonne[col]=sumcolonne[col]+1;
            }
        }
    }
    for index in 0..i{
        let int=sumcolonne[index] as f32;
        let ou=sumligne[index] as f32;
        //println!("{} {}",int,ou);
        out.push(int/ou);
    }
    out
}