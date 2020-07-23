use crate::classical::state::*;
use crate::classical::{GroundProblem};
use crate::explain::state2::*;
use crate::explain::explain::*;
use nalgebra::base::*;

//Quelles sont les supports de l’étape A?
pub fn Question1(num:usize,support : &DMatrix<i32>,plan: &Vec<Op>)->Vec<Resume>{
    let t= support.nrows();
    let out = Vec::new();
    for i in 0..t{
        if support[(i,num)] == 1 {
            if !plan.get(i).is_none(){
                let n=newresume(plan.get(i).unwrap(),i);
                out.push(n);
            }
        }
    }
    out
}

pub fn affichageQ1<T,I : Display>(num:usize, plan:&Vec<Op>, sup:Vec<Resume>,symbol:World<T,I>){
    let n=newresume(plan.get(num).unwrap(),num);
    println!("L'opérateur {} de l'étape {} est supporté par ",symbol.table.format(&ground.operators.name(plan.get(num).unwrap())),num);
    for i in sup{
        print!(" l'opérateur {} de l'étape {}, ",symbol.table.format(&ground.operators.name(i.op())) ,i.numero());
    }
}

//Quelles sont les actions supportés par l’étape A?
pub fn Question2(num:usize,support : &DMatrix<i32>,plan: &Vec<Op>)->Vec<Resume>{
    let t= support.nrows();
    let out = Vec::new();
    for i in 0..t{
        if support[(num,i)] == 1 {
            if !plan.get(i).is_none(){
                let n=newresume(plan.get(i).unwrap(),i);
                out.push(n);
            }
        }
    }
    out
}

pub fn affichageQ2<T,I : Display>(num:usize, plan:&Vec<Op>, sup:Vec<Resume>,symbol:World<T,I>){
    let n=newresume(plan.get(num).unwrap(),num);
    println!("L'opérateur {} de l'étape {} supporte ",symbol.table.format(&ground.operators.name(plan.get(num).unwrap())),num);
    for i in sup{
        print!(" l'opérateur {} de l'étape {}, ",symbol.table.format(&ground.operators.name(i.op())) ,i.numero());
    }
}

//Est-ce que l’execution de A avant B peux gêner l’execution de B? 
pub fn Question3 (A:usize,B:usize,menace:&DMatrix<i32>)->bool{
    let mut b=true;
    if menace[(A,B)]==0{
        b=false;
    }
    b
}

pub fn affichageQ3<T,I : Display>(A:usize,B:usize,m:bool, plan:&Vec<Op>,symbol:World<T,I>){
    if m{
        println!("L'opérateur {} de l'étape {} menace l'opérateur {} de l'étape {} ",symbol.table.format(&ground.operators.name(plan.get(A).unwrap())),A,symbol.table.format(&ground.operators.name(plan.get(B).unwrap())),B);
    }else{
        println!("L'opérateur {} de l'étape {} ne menace pas l'opérateur {} de l'étape {} ",symbol.table.format(&ground.operators.name(plan.get(A).unwrap())),A,symbol.table.format(&ground.operators.name(plan.get(B).unwrap())),B);
    }
}

//Est-ce que cette étape est nécessaire? Participe-t-elle à l’accomplissement d’un but?
pub fn Question4(num:usize, support : &DMatrix<i32>, plan:&Vec<Op>,ground: &GroundProblem)->bool{
    let allnec=dijkstra2(support,plan,ground);
    let nec= newnecess();
    for i in allnec{
        if i.opnec().numero()==num{
            nec=i;
        }
    }
    nec.nec()
}

pub fn QuestionDetail4(num:usize, support : &DMatrix<i32>, plan:&Vec<Op>,ground: &GroundProblem)->Option<Vec<Resume>>{
    let allnec=dijkstra2(support,plan,ground);
    let nec= newnecess();
    for i in allnec{
        if i.opnec().numero()==num{
            nec=i;
        }
    }
    nec.chemin()
}

pub fn affichageQ4 (num:usize,b:bool,plan: &Vec<Op>,symbol:World<T,I>){
    if b{
        println!(" L'opérateur {} de l'étape {} est  nécessaire",symbol.table.format(&ground.operators.name(plan.get(num).unwrap())),num );
    }else{
        println!(" L'opérateur {} de l'étape {} est  nécessaire",symbol.table.format(&ground.operators.name(plan.get(num).unwrap())),num );
    }
}
//a refaire sans nec et avec option Vec
pub fn affichageQD4 (n:Necessaire,plan: &Vec<Op>,symbol:World<T,I>){
    print!("L'operateur {} de ",symbol.table.format(&ground.operators.name(n.opnec().op())));
    n.affiche();
}

//Existe-t-il un chemin entre A et B?
pub fn  Question5 (A:usize, B:usize, support : &DMatrix<i32>, plan:&Vec<Op>)->bool{
    let step1= A as i32;
    let Step2 = B as i32;
    if step1 > step2 {
        let nec=explicationsupport(plan, support, /*ground,*/ step1, step2);
    }else{
        let nec=explicationsupport(plan, support, /*ground,*/ step2, step1);
    };
    nec.nec()
}

pub fn  QuestionDetail5(A:usize, B:usize, support : &DMatrix<i32>, plan:&Vec<Op>)->Option<Vec<Resume>>{
    let step1= A as i32;
    let Step2 = B as i32;
    if step1 > step2 {
        let nec=explicationsupport(plan, support, /*ground,*/ step1, step2);
    }else{
        let nec=explicationsupport(plan, support, /*ground,*/ step2, step1);
    };
    nec.nec()
}

pub fn affichageQ5 (A:usize,B:usize,b:bool,plan: &Vec<Op>,symbol:World<T,I>){
    if b{
        println!(" L'opérateur {} de l'étape {} et l'opérateur {} de l'étape {} sont liés par un chemin dans le graph de support",symbol.table.format(&ground.operators.name(plan.get(A).unwrap())),A ,symbol.table.format(&ground.operators.name(plan.get(B).unwrap())),B );
    }else{
        println!(" L'opérateur {} de l'étape {} et l'opérateur {} de l'étape {} ne sont pas liés par un chemin dans le graph de support",symbol.table.format(&ground.operators.name(plan.get(A).unwrap())),A ,symbol.table.format(&ground.operators.name(plan.get(B).unwrap())),B );
    }
}

//a refaire sans nec et avec option Vec
pub fn affichageQD5 (n:Necessaire,plan: &Vec<Op>,symbol:World<T,I>){
    print!("L'operateur {} de ",symbol.table.format(&ground.operators.name(n.opnec().op())));
    n.affiche();
}

//Est-ce que les étapes A et B sont parallélisable?
pub fn Question6(){

}

