use crate::classical::state::*;
use crate::classical::{GroundProblem};
use crate::symbols::SymbolTable;
use crate::explain::state2::*;
use crate::explain::explain::*;
use nalgebra::base::*;
use std::fmt::{Display, Error, Formatter};

//Quelles sont les supports de l’étape a?
pub fn supportedby(num:usize,support : &DMatrix<i32>,plan: &Vec<Op>)->Vec<Resume>{
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
        print!("    l'opérateur {} de l'étape {}, ",symbol.table.format(&ground.operators.name(i.op().unwrap())) ,i.numero());
    }
    println!("");
}

//Quelles sont les actions supportés par l’étape a?
pub fn supportof(num:usize,support : &DMatrix<i32>,plan: &Vec<Op>)->Vec<Resume>{
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
        println!("  l'opérateur {} de l'étape {}, ",symbol.table.format(&ground.operators.name(i.op().unwrap())) ,i.numero());
    }
    println!("");
}

//Est-ce que l’execution de a avant b peux gêner l’execution de b? 
pub fn menacefromto (a:usize,b:usize,menace:&DMatrix<i32>)->bool{
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
pub fn isnecessary(num:usize, support : &DMatrix<i32>, plan:&Vec<Op>,ground: &GroundProblem)->bool{
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

pub fn isnecessarydetail(num:usize, support : &DMatrix<i32>, plan:&Vec<Op>, ground: &GroundProblem)->Option<Vec<Resume>>{
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
/*pub fn affichageqd4 (n:Necessaire, ground: &GroundProblem ,symbol:&World<String,String>){
    print!("L'operateur {} de ",symbol.table.format(&ground.operators.name(n.opnec().op().unwrap())));
    n.affiche();
    println!("");
}*/

pub fn affichageqd4 (num:usize,chemin:Option<Vec<Resume>>,plan : &Vec<Op> ,ground: &GroundProblem ,symbol:&World<String,String>){
    print!("L'operateur {} de ",symbol.table.format(&ground.operators.name(*plan.get(num).unwrap())));
    if chemin.is_none(){
        println!("n'est pas necessaire")
    }
    else{
        println!("est necessaire pour notamment le chemin accomplissant un but composé par :");
        for op in chemin.unwrap(){
            println!(" l'étape {}", op.numero());
        }
    }
    println!("");
}

//Existe-t-il un chemin entre a et b?
pub fn  waybetweenbool (a:usize, b:usize, support : &DMatrix<i32>, plan:&Vec<Op>)->bool{
    /*let step1= a as i32;
    let step2 = b as i32;
    let mut nec;
    if step1 > step2 {
        nec=explicationsupport(plan, support, step1, step2);
    }else{
        nec=explicationsupport(plan, support,  step2, step1);
    };
    nec.nec()*/
    waybetween(a,b,support,plan).is_some()
}

pub fn  waybetween(a:usize, b:usize, support : &DMatrix<i32>, plan:&Vec<Op>)->Option<Vec<Resume>>{
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
/*pub fn affichageqd5  (n:Necessaire, ground:&GroundProblem,symbol:&World<String,String>){
    print!("L'operateur {} de ",symbol.table.format(ground.operators.name(n.opnec().op().unwrap())));
    n.affiche();
    println!("");
}*/
pub fn affichageqd5  (n: &Option<Vec<Resume>>, ground:&GroundProblem,symbol:&World<String,String>){
    let a =n.clone();
    println!("Le chemin est composé :");
    if a.is_some(){
        for i in a.unwrap(){
            println!("  de l'operateur {} de l'étape {} ,",symbol.table.format(ground.operators.name(i.op().unwrap())),i.numero());
        }
    }else{
        println!("Pas de chemin");
    }
}

//Est-ce que les étapes a et b sont parallélisable? privilege support
pub fn parallelisablebool(a:usize,b:usize, support : &DMatrix<i32>, menace:&DMatrix<i32>,plan: &Vec<Op>,ground:&GroundProblem)->Parallelisable{
    /*let mut p: Parallelisable = Parallelisable::Oui;
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
    p*/
    let qd=parallelisable(a,b,support,menace,plan,ground);
    /*if qd == Parallelisabledetail::Oui{ return Parallelisable::Oui}
    else if qd == Parallelisabledetail::Support_Direct{origine : a,vers:b} || qd==Parallelisabledetail::Support_Indirect{origine:a,vers:b,chemin}{
        return Parallelisable::Non_support{origine:a,vers:b}
    }
    else if qd == Parallelisabledetail::Support_Direct{origine : b,vers:a} || qd==Parallelisabledetail::Support_Indirect{origine:b,vers:a,chemin}{
        return Parallelisable::Non_support{origine:b,vers:a}
    }
    else{return Parallelisable::Non_menace{origine:a,vers:b}}*/
    match qd{
        Parallelisabledetail::Menace_Avant {origine,vers,supportconcern}=> return Parallelisable::Non_menace{origine,vers},
        Parallelisabledetail::Menace_Apres {origine,vers}=> return Parallelisable::Non_menace{origine,vers},
        Parallelisabledetail::Support_Direct {origine,vers}=> return Parallelisable::Non_support{origine,vers},
        Parallelisabledetail::Support_Indirect {origine,vers,chemin}=> return Parallelisable::Non_support{origine,vers},
        Parallelisabledetail::Oui=> return Parallelisable::Oui,
    }
    
}

pub fn parallelisable(a:usize,b:usize, support : &DMatrix<i32>, menace:&DMatrix<i32>,plan: &Vec<Op>,ground:&GroundProblem)->Parallelisabledetail{
    let mut p= Parallelisabledetail::Oui;
    let ai = a as i32;
    let bi = b as i32;
    if a > b {

        if support[(b,a)]==1{
            p= Parallelisabledetail::Support_Direct{origine:b,vers:a};
        }
        else{
            let nec=explicationsupport(plan, support, ai, bi);
            if nec.nec(){
                p= Parallelisabledetail::Support_Indirect{origine:a,vers:b,chemin:nec.chemin()};
            }
        }
        
    }else{
        if support[(a,b)]==1{
            p= Parallelisabledetail::Support_Direct{origine:a,vers:b};
        }
        else{
            let nec=explicationsupport(plan, support, bi, ai);
            if nec.nec(){
            p= Parallelisabledetail::Support_Indirect{origine:a,vers:b,chemin:nec.chemin()};
            }
        }
    }
    if p==Parallelisabledetail::Oui{
        let opt=explicationmenacequestiondetail(plan,menace,support,ai,bi);
        if opt.is_some(){
            let (s1,s2,i)=opt.unwrap();
            if i.is_some(){
                p=Parallelisabledetail::Menace_Avant{origine:s1,vers:s2,supportconcern:i}
            }else{
                p=Parallelisabledetail::Menace_Apres{origine:s1,vers:s2};
            }
        }
        
        let opt=explicationmenacequestiondetail(plan,menace,support,bi,ai);
        if opt.is_some(){
            let (s1,s2,i)=opt.unwrap();
            if i.is_some(){
                p=Parallelisabledetail::Menace_Avant{origine:s1,vers:s2,supportconcern:i}
            }else{
                p=Parallelisabledetail::Menace_Apres{origine:s1,vers:s2};
            }
        }

    }
    p
}

pub fn affichageq6(p : Parallelisable){
    match p{
        Parallelisable::Oui=>println!("est parallelisable "),
        Parallelisable::Non_menace{origine,vers}=> println!(" n'est pas parallelisable car il y a une menace "),
        Parallelisable::Non_support{origine,vers}=> println!("n'est pas parallelisable car il y a une relation de support "),
    }
}

pub fn affichageqd6 (p : Parallelisabledetail){
    match p{
        Parallelisabledetail::Oui=> println!("est parallelisable"),
        Parallelisabledetail::Support_Direct{origine,vers}=> println!("n'est pas parallelisable car il y a relation de support direct"),
        Parallelisabledetail::Support_Indirect{origine,vers,chemin}=> println!("N'est pas parallelisable car il a une relation de support indirect "),
        Parallelisabledetail::Menace_Apres{origine,vers}=> println!("N'est pas parallelisable car l'étape la plus récente menace l'étape antérieur "),
        Parallelisabledetail::Menace_Avant{origine,vers,supportconcern}=> println!("N'est pas parallelisable car l'étape antérieur menace l'étape plus récente ")
    }
}

//L’action accomplit-elle directement un goal?
pub fn achievegoal (num: usize,support : &DMatrix<i32>)->bool{
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

pub fn weightwaygoal (num: usize,exclusion:usize ,support : &DMatrix<i32>,plan: &Vec<Op>,ground: &GroundProblem,poids:i32)->bool{
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

pub fn weightwaygoal2(num:usize,action:String, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->Option<Vec<Resume>>{
    let exclu=choixpredaction3(action,plan,ground,wo);
    let necs=dijkstrapoids(plan ,ground,support ,&exclu,poids );
    let mut out;
    let n=num as i32;
    let r =newresume(*plan.get(num).unwrap(), n);
    let nec= newnecess(r);
    out=nec.chemin();
    for i in necs{
//        i.affiche();
        if i.opnec().numero()==n{
            out=i.chemin();
        }
    }
    out
}

pub fn weightway(step1: usize,step2:usize, action:String, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->bool{
    let out = weightwaydetail(step1,step2, action, support, plan, ground,wo,poids);
    out.is_some()
}


pub fn weightwaydetail(step1: usize,step2:usize, action:String, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->Option<Vec<Resume>>{
    let s1=step1 as i32;
    let s2=step2 as i32;
    let exclu=choixpredaction3(action,plan,ground,wo);
    let necs = supportindirectpoid(s1,s2,plan,ground,support,&exclu,poids);
    necs.chemin()
}

pub fn inverseweightway(step1: usize,step2:usize, action:String, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->bool{
    let out = inverseweightwaydetail(step1,step2, action, support, plan, ground,wo,poids);
    out.is_some()
}


pub fn inverseweightwaydetail(step1: usize,step2:usize, action:String, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->Option<Vec<Resume>>{
    let s1=step1 as i32;
    let s2=step2 as i32;
    let exclu=choixpredaction3(action,plan,ground,wo);
    let necs = supportindirectavantagepoid(s1,s2,plan,ground,support,&exclu,poids);
    necs.chemin()
}
//en utilisanst le num d'étapes
pub fn weightwayetape(step1: usize,step2:usize, step:usize, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->bool{
    let out = weightwaydetailetape(step1,step2, step, support, plan, ground,wo,poids);
    out.is_some()
}


pub fn weightwaydetailetape(step1: usize,step2:usize, step:usize, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->Option<Vec<Resume>>{
    let s1=step1 as i32;
    let s2=step2 as i32;
    let exclu=choixpredaction2(step,plan,ground);
    let necs = supportindirectpoid(s1,s2,plan,ground,support,&exclu,poids);
    necs.chemin()
}

pub fn inverseweightwayetape(step1: usize,step2:usize, step:usize, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->bool{
    let out = inverseweightwaydetailetape(step1,step2, step, support, plan, ground,wo,poids);
    out.is_some()
}


pub fn inverseweightwaydetailetape(step1: usize,step2:usize, step:usize, support : &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, wo: &SymbolTable<String,String>,poids:i32)->Option<Vec<Resume>>{
    let s1=step1 as i32;
    let s2=step2 as i32;
    let exclu=choixpredaction2(step,plan,ground);
    let necs = supportindirectavantagepoid(s1,s2,plan,ground,support,&exclu,poids);
    necs.chemin()
}

pub fn affichageq9d(chemin : &Option<Vec<Resume>>, ground: &GroundProblem,wo: &SymbolTable<String,String>){
    if chemin.is_some(){
        let n = chemin.clone();
        println!("Le chemin entre les 2 étapes est composé par :");
        for i in n {
            for step in i{
                println!("l'opérateur {} de l'étapes {} ",wo.format(&ground.operators.name(step.op().unwrap())),step.numero());
            }
        }
    }else{
        println!("les étapes ne sont pas liés par une relation de support!");
    }
}


