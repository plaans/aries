use crate::classical::heuristics::*;
use crate::classical::state::*;
use crate::classical::{GroundProblem};
use crate::classical::state2::*;
use std::fmt::Display;
use nalgebra::base::*;
use std::collections::HashMap;

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

pub fn calculcentraliteglobal2(support : &DMatrix<i32>)->Vec<(usize,usize)>{
    
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
        /*let int=sumcolonne[index] as f32;
        let ou=sumligne[index] as f32;
        //println!("{} {}",int,ou);*/
        out.push((sumcolonne[index],sumligne[index]));
    }
    out
}

pub fn regroupementcentralite (centra: &Vec<f32>,plan: &Vec<Op>)->(Vec<f32>,HashMap<usize,Vec<Resume>>){
    let taille=centra.len();
    let mut val=Vec::new();
    let mut regroup = HashMap::new();
    //fausse hmap avec les floats
    for i in centra{
        let mut boolean =false;
        for v in &val{
                if *v==*i{
                    boolean =true;
                }
        }
        if boolean == false{
            val.push(*i);
        }
    }
    //Hmap
    for i in 0..taille{
        if !plan.get(i).is_none(){
            let index = i as i32;
            //crea du resume de l'étape
            let r=newresume(*plan.get(i).unwrap(), index);
            let mut key=0;
            for i2 in 0..val.len(){
                //si la valeur existe
                if !val.get(i).is_none(){ 
                    if *val.get(i2).unwrap() == *centra.get(i).unwrap(){
                        key = i2;
                        println!("===--------==== C3 k{},v{},c{}",key,*val.get(i2).unwrap(),*centra.get(i).unwrap());
                    }    
                }
                else{
                    val.push(*centra.get(i).unwrap());
                    key= val.len();
                    println!("=====--------------------------== C4 {}", key);
                }
                
            }
            let essai= regroup.get_mut(&key);
            println!("{}",key);
            if essai.is_none(){
                let mut v=Vec::new();
                v.push(r);
                regroup.insert(key,v);
                println!("====----------       -----     --------=== C5 pas normal {}", key);
            }else{
                let v=essai.unwrap();
                v.push(r);
                println!("======= C6 {}", key);
            }
        }
    }
    (val,regroup)
}



pub fn regroupementcentraliteaction (centra: &Vec<f32>,plan: &Vec<Op>)->HashMap<Op,Vec<f32>>{
    let taille=centra.len();
    let mut regroup = HashMap::new();
    for i in 0..taille{
        if !plan.get(i).is_none(){
            if regroup.get_mut(plan.get(i).unwrap()).is_none(){
                let mut  v=Vec::new();
                v.push(*centra.get(i).unwrap());
                regroup.insert(*plan.get(i).unwrap(),v);
            }else{
                let essai= regroup.get_mut(plan.get(i).unwrap()).unwrap();
                essai.push(*centra.get(i).unwrap());
            }
        }
    }
    regroup
}

pub fn affichagehmapaction(val:HashMap<Op,Vec<f32>>){
    for (i,v) in val.iter(){
        println!("{:?} de centralité : ",*i);
        for n in v{
            print!("{}, ", *n);
        }
        println!("");
    }
}

pub fn affichageregroucentra(key : Vec<f32>,val:HashMap<usize,Vec<Resume>>){
    println!("======= SUUUUU {}",key.len());
    for i in 0..key.len(){
        println!("======= centralite {}",key.get(i).unwrap());
        for d in val.get(&i){
            for r in d{
                println!("L'opérateur {:?} de l'étape {}",r.op(),r.numero());
            }
            
        }
    }
}