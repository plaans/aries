//use crate::classical::heuristics::*;
use crate::classical::state::*;
use crate::symbols::{SymbolTable,SymId};
use crate::classical::{GroundProblem};
use crate::explain::state2::*;
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

pub fn regroupementcentralite (centra: &Vec<(usize,usize)>,plan: &Vec<Op>)->HashMap<(usize,usize),Vec<Resume>>{
    let taille=centra.len();
    let mut regroup = HashMap::new();
    //Hmap
    for i in 0..taille{
        if !plan.get(i).is_none(){
            let index = i as i32;
            //crea du resume de l'étape
            let r=newresume(*plan.get(i).unwrap(), index);
            let mut key = *centra.get(i).unwrap();
            //ajout condition if (n,n)->(1,1)
            let (a,b)= *centra.get(i).unwrap();
            if a==b{
                //print!("{:?}",key);
                key= (1,1);
                //println!("chngmt key")
            }
            let essai= regroup.get_mut(&key);
            //println!("{:?}",key);
            if essai.is_none(){
                let mut v=Vec::new();
                v.push(r);
                regroup.insert(key,v);
                //println!("====----------       -----     --------=== C5 pas normal {:?}", key);
            }else{
                let v=essai.unwrap();
                v.push(r);
                //println!("======= C6 {:?}", key);
            }
        }
    }
    regroup
}



pub fn regroupementcentraliteop(centra: &Vec<f32>,plan: &Vec<Op>)->HashMap<Op,Vec<f32>>{
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

pub fn regroupementcentraliteaction (centra: &Vec<f32>,plan: &Vec<Op>, ground: &GroundProblem, symbol : &SymbolTable<String,String>)->HashMap<SymId,Vec<f32>>{
    let taille=centra.len();
    let mut regroupe = HashMap::new();

    //Compter nombre D'action (SymId même principe que Op)
    let mut nbop=0;
    let mut v=Vec::new();
    for i in plan{
        if v.is_empty(){
            let action = ground.operators.name(*i)[0];
            v.push(action);
            let mut vec=Vec::new();
            regroupe.insert(action,vec);
            nbop=nbop+1;
        }else{
            let mut notin=true;
            for ope in &v{
                let action=&ground.operators.name(*i);
                if *ope == action[0] {
                    
                    notin=false;
                }
            }
            if notin {
                let action = ground.operators.name(*i)[0];
                v.push(action);
                let mut vec=Vec::new();
                regroupe.insert(action,vec);
                nbop=nbop+1;   
            }
        }
    }

    for index in 0..taille{
        if !plan.get(index).is_none(){
            let action =ground.operators.name(*plan.get(index).unwrap())[0];
            if regroupe.get_mut(&action).is_none(){
                let mut  v=Vec::new();
                v.push(*centra.get(index).unwrap());
                regroupe.insert(action,v);
            }else{
                let essai= regroupe.get_mut(&action).unwrap();
                essai.push(*centra.get(index).unwrap());
            }
        }
    }
    
    regroupe
}

pub fn affichagehmapop<T,I : Display>(val:HashMap<Op,Vec<f32>>,ground: &GroundProblem,symbol: &World<T,I> ){
    for (i,v) in val.iter(){
        print!("L'opérateur {} numéroté ",symbol.table.format(&ground.operators.name(*i)));
        println!("{:?} de centralité : ",*i);
        
        for n in v{
            print!("{}, ", *n);
        }
        println!("");
    }
}

pub fn affichagehmapaction<T,I : Display>(val:HashMap<SymId,Vec<f32>>,symbol: &World<T,I> ){
    for (i,v) in val.iter(){
        let vecinter = vec![*i];
        let slice = &vecinter[..];
        println!("L'action {} de centralité :",symbol.table.format(slice));        
        for n in v{
            print!("{}, ", *n);
        }
        println!("");
    }
}

pub fn affichageregroucentra<T,I : Display>(val:HashMap<(usize,usize),Vec<Resume>>,ground: &GroundProblem,symbol: &World<T,I> ){
    println!("======= SUUUUU {}",val.len());
    for i in val.keys(){
        println!("======= centralite {:?}",i);
        for d in val.get(&i){
            for r in d{
                print!("L'opérateur {:?} de l'étape {} alias ",r.op(),r.numero());
                println!("L'opérateur {} de l'étape {}",symbol.table.format(&ground.operators.name(r.op().unwrap())),r.numero());
            }
            
        }
    }
}


