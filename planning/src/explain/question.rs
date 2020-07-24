use crate::classical::state::*;
use crate::classical::{GroundProblem};
use crate::symbols::SymbolTable;
use crate::explain::state2::*;
use crate::explain::explain::*;
use nalgebra::base::*;
use std::fmt::{Display, Error, Formatter};

//Quelles sont les supports de l’étape a?
pub fn question1(num:usize,support : &DMatrix<i32>,plan: &Vec<Op>)->Vec<Resume>{
    let t= support.nrows();
    let mut out = Vec::new();
    for i in 0..t{
        if support[(i,num)] == 1 {
            if !plan.get(i).is_none(){
                let u=i as i32;
                let n=newresume(*plan.get(i).unwrap(),u);
                out.push(n);
            }
        }
    }
    out
}

pub fn affichageq1 (num:usize, plan:&Vec<Op>, sup:Vec<Resume>, ground: &GroundProblem, symbol:&World<String,String>){
    //let i=num as i32;
    //let n=newresume(*plan.get(num).unwrap(),i);
    println!("L'opérateur {} de l'étape {} est supporté par ",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())),num);
    for i in sup{
        print!(" l'opérateur {} de l'étape {}, ",symbol.table.format(&ground.operators.name(i.op().unwrap())) ,i.numero());
    }
    println!("");
}

//Quelles sont les actions supportés par l’étape a?
pub fn question2(num:usize,support : &DMatrix<i32>,plan: &Vec<Op>)->Vec<Resume>{
    let t= support.nrows();
    let mut out = Vec::new();
    for i in 0..t{
        if support[(num,i)] == 1 {
            if !plan.get(i).is_none(){
                let u=i as i32;
                let n=newresume(*plan.get(i).unwrap(),u);
                out.push(n);
            }
        }
    }
    out
}

pub fn affichageq2 (num:usize, plan:&Vec<Op>, sup:Vec<Resume>, ground: &GroundProblem, symbol:&World<String,String>){
    //let i = num as i32;
    //let n=newresume(*plan.get(num).unwrap(),i);
    println!("L'opérateur {} de l'étape {} supporte ",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())),num);
    for i in sup{
        print!(" l'opérateur {} de l'étape {}, ",symbol.table.format(&ground.operators.name(i.op().unwrap())) ,i.numero());
    }
    println!("");
}

//Est-ce que l’execution de a avant b peux gêner l’execution de b? 
pub fn question3 (a:usize,b:usize,menace:&DMatrix<i32>)->bool{
    let mut bo=true;
    if menace[(a,b)]==0{
        bo=false;
    }
    bo
}

pub fn affichageq3(a:usize,b:usize,m:bool, plan:&Vec<Op>, ground: &GroundProblem, symbol:&World<String,String>){
    if m{
        println!("L'opérateur {} de l'étape {} menace l'opérateur {} de l'étape {} ",symbol.table.format(&ground.operators.name(*plan.get(a).unwrap())),a,symbol.table.format(&ground.operators.name(*plan.get(b).unwrap())),b);
    }else{
        println!("L'opérateur {} de l'étape {} ne menace pas l'opérateur {} de l'étape {} ",symbol.table.format(&ground.operators.name(*plan.get(a).unwrap())),a,symbol.table.format(&ground.operators.name(*plan.get(b).unwrap())),b);
    }
}

//Est-ce que cette étape est nécessaire? Participe-t-elle à l’accomplissement d’un but?
pub fn question4(num:usize, support : &DMatrix<i32>, plan:&Vec<Op>,ground: &GroundProblem)->bool{
    let allnec=dijkstra2(support,plan.clone(),ground);
    let i = num as i32;
    let r =newresume(*plan.get(num).unwrap(), i);
    let mut nec= newnecess(r);
    for n in allnec{
        if n.opnec().numero()==i{
            nec=n;
        }
    }
    nec.nec()
}

pub fn questiondetail4(num:usize, support : &DMatrix<i32>, plan:&Vec<Op>, ground: &GroundProblem)->Option<Vec<Resume>>{
    let allnec=dijkstra2(support,plan.clone(),ground);
    let i = num as i32;
    let r=newresume(*plan.get(num).unwrap(), i);
    let mut nec= newnecess(r);
    for n in allnec{
        if n.opnec().numero()==i{
            nec=n;
        }
    }
    nec.chemin()
}

pub fn affichageq4(num:usize,b:bool,plan: &Vec<Op>, ground: &GroundProblem, symbol:&World<String,String>){
    if b{
        println!(" L'opérateur {} de l'étape {} est  nécessaire",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())),num );
    }else{
        println!(" L'opérateur {} de l'étape {} est  nécessaire",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())),num );
    }
}
//a refaire sans nec et avec option Vec
pub fn affichageqd4 (n:Necessaire, ground: &GroundProblem ,symbol:&World<String,String>){
    print!("L'operateur {} de ",symbol.table.format(&ground.operators.name(n.opnec().op().unwrap())));
    n.affiche();
    println!("");
}

//Existe-t-il un chemin entre a et b?
pub fn  question5 (a:usize, b:usize, support : &DMatrix<i32>, plan:&Vec<Op>)->bool{
    let step1= a as i32;
    let step2 = b as i32;
    let mut nec;
    if step1 > step2 {
        nec=explicationsupport(plan, support, step1, step2);
    }else{
        nec=explicationsupport(plan, support,  step2, step1);
    };
    nec.nec()
}

pub fn  questiondetail5(a:usize, b:usize, support : &DMatrix<i32>, plan:&Vec<Op>)->Option<Vec<Resume>>{
    let step1= a as i32;
    let step2 = b as i32;
    let mut nec;
    if step1 > step2 {
        nec=explicationsupport(plan, support,  step1, step2);
    }else{
        nec=explicationsupport(plan, support,  step2, step1);
    }
    nec.chemin()
}

pub fn affichageq5 (a:usize,b:usize,bo:bool,plan: &Vec<Op>, ground:&GroundProblem, symbol:&World<String,String>){
    if bo{
        println!(" L'opérateur {} de l'étape {} et l'opérateur {} de l'étape {} sont liés par un chemin dans le graph de support",symbol.table.format(&ground.operators.name(*plan.get(a).unwrap())),a ,symbol.table.format(&ground.operators.name(*plan.get(b).unwrap())),b );
    }else{
        println!(" L'opérateur {} de l'étape {} et l'opérateur {} de l'étape {} ne sont pas liés par un chemin dans le graph de support",symbol.table.format(&ground.operators.name(*plan.get(a).unwrap())),a ,symbol.table.format(&ground.operators.name(*plan.get(b).unwrap())),b );
    }
}

//a refaire sans nec et avec option Vec
pub fn affichageqd5  (n:Necessaire, ground:&GroundProblem,symbol:&World<String,String>){
    print!("L'operateur {} de ",symbol.table.format(ground.operators.name(n.opnec().op().unwrap())));
    n.affiche();
    println!("");
}

//Est-ce que les étapes a et b sont parallélisable? privilege support
pub fn question6(a:usize,b:usize, support : &DMatrix<i32>, menace:&DMatrix<i32>,plan: &Vec<Op>,ground:&GroundProblem)->Parallelisable{
    let mut p: Parallelisable = Parallelisable::Oui;
    let ai = a as i32;
    let bi = b as i32;
    if a > b {
        let nec=explicationsupport(plan, support, ai, bi);
        if nec.nec(){p= Parallelisable::Non_support{origine:a,vers:b};}
    }else{
        let nec=explicationsupport(plan, support, bi, ai);
        if nec.nec(){p= Parallelisable::Non_support{origine:b,vers:a};}
    }
    if p == Parallelisable::Oui{
        let m =explicationmenacequestion(plan, menace, support, ai, bi);
        if m{
          p=Parallelisable::Non_menace{origine:a,vers:b};  
        }
        let m=explicationmenacequestion(plan, menace, support,  bi, ai);
        if m{
            p=Parallelisable::Non_menace{origine:b,vers:a};  
        }
    }  
    p
}

pub fn questiondetail6(a:usize,b:usize, support : &DMatrix<i32>, menace:&DMatrix<i32>,plan: &Vec<Op>,ground:&GroundProblem)->Parallelisabledetail{
    let mut p= Parallelisabledetail::Oui;
    p
}

//L’action accomplit-elle directement un goal?
pub fn question7 (num: usize,support : &DMatrix<i32>)->bool{
    let t = support.nrows();
    let mut g=false;
    if support[(num,t-2)]== 1{
        g=true;
    }
    g
}

pub fn affichageq7 (num: usize,b:bool,plan: &Vec<Op>,ground:&GroundProblem, symbol:&World<String,String>){
    if b{
        println!("L'opérateur {} de l'étape {} accomplit un but ",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())),num);
    }else{
        println!("L'opérateur {} de l'étape {} n'accomplit pas de but",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())),num);
    }
    
}

pub fn question9g (num: usize,exclusion:usize ,support : &DMatrix<i32>,plan: &Vec<Op>,ground: &GroundProblem,poids:i32)->bool{
    let exclu=choixpredaction2(exclusion,plan,ground);
    let necs=dijkstrapoids(plan ,ground,support ,&exclu,poids );
    let mut b = false;
    let n=num as i32;
    for i in necs{
        if i.opnec().numero()==n{
            b=true;
        }
    }
    b
}

pub fn question9g2(num:usize,action:String, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->Option<Vec<Resume>>{
    let exclu=choixpredaction3(action,plan,ground,wo);
    let necs=dijkstrapoids(plan ,ground,support ,&exclu,poids );
    let mut out;
    let n=num as i32;
    let r =newresume(*plan.get(num).unwrap(), n);
    let nec= newnecess(r);
    out=nec.chemin();
    for i in necs{
        if i.opnec().numero()==n{
            out=i.chemin();
        }
    }
    out
}

